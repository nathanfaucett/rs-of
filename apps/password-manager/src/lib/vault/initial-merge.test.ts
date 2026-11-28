import { Database } from "@ofsdk/sql";
import { expect, it } from "vitest";
import { createEncryptedVault, unlockEncryptedVault } from "./encrypted-vault";
import { vaultHistoryRecords } from "./history-records";
import {
  clearMerge,
  encodeRevision,
  readPendingMerge,
  reopenCandidate,
  retainCandidate,
} from "./merge-history";
import type { Secret } from "./secrets/secret";
import { dbParam, MIGRATIONS, parseText } from "./storage/schema";
import { localDatabase } from "./vault";

it("reopens independent same-name Vaults with their original encrypted records and deletions", async () => {
  const names = [`left-${crypto.randomUUID()}`, `right-${crypto.randomUUID()}`];
  const sources = await Promise.all(names.map((name) => Database.open(name)));
  try {
    const fixtures = [];
    for (const [index, source] of sources.entries()) {
      const password = `fixture password ${index}`;
      const client = source.client();
      try {
        await client.migrate(MIGRATIONS);
        const vault = await createEncryptedVault(localDatabase(client), password, "Same name");
        try {
          const folder = await vault.createFolder(`Private folder ${index}`);
          const now = "2026-01-01T00:00:00.000Z";
          const item = {
            id: crypto.randomUUID(),
            kind: "secureNote" as const,
            type: 0 as const,
            name: `Private note ${index}`,
            notes: `Secret ${index}`,
            createdAt: now,
            updatedAt: now,
            favorite: false,
            folderId: folder.id,
            fields: [],
            history: [],
          };
          const deleted = { ...item, id: crypto.randomUUID(), notes: "Deleted secret" };
          await vault.mutateItems({ upsert: [item, deleted] });
          await vault.mutateItems({ deleteIds: [deleted.id] });
          await vault.saveLockSettings({ timeoutMinutes: index === 0 ? 1 : 15 });
          fixtures.push({ id: vault.id, password, item, folder, deleted: deleted.id });
        } finally {
          await vault.close();
        }
      } finally {
        client.close();
      }
    }
    const [left, right] = sources;
    if (!left || !right) throw new Error("Two source databases required");
    const pending = await retainFixture(left, right);
    expect(readPendingMerge()).toEqual(pending);
    const candidate = await reopenCandidate(pending);
    try {
      const client = candidate.client();
      try {
        expect((await client.migrate(MIGRATIONS)).appliedVersions).toEqual([]);
        const ids = await client.execute("SELECT id FROM vaults ORDER BY id", (row) =>
          parseText(row, "id")
        );
        expect(ids).toEqual(fixtures.map(({ id }) => id).sort());
        for (const { id, password, item, folder, deleted } of fixtures) {
          const vault = await unlockEncryptedVault(localDatabase(client), password, id);
          if (!vault) throw new Error("Staging lost a Vault");
          try {
            const { items, folders } = await vault.readItems();
            expect(items).toHaveLength(1);
            expect(items[0]).toMatchObject(item);
            expect(items.map((row) => row.id)).not.toContain(deleted);
            expect(folders).toEqual([folder]);
            expect(await vault.loadLockSettings()).toEqual({
              timeoutMinutes: id === fixtures[0]?.id ? 1 : 15,
            });
          } finally {
            await vault.close();
          }
        }
      } finally {
        client.close();
      }
    } finally {
      candidate.close();
      clearMerge(pending);
    }
    for (const [index, source] of sources.entries()) {
      const client = source.client();
      try {
        expect(
          await client.execute("SELECT id FROM vaults", (row) => parseText(row, "id"))
        ).toEqual([fixtures[index]?.id]);
      } finally {
        client.close();
      }
    }
  } finally {
    for (const source of sources) source.close();
    for (const name of names) {
      await new Promise<void>((resolve, reject) => {
        const request = indexedDB.deleteDatabase(name);
        request.onsuccess = () => resolve();
        request.onerror = () => reject(request.error);
        request.onblocked = () => reject(new Error("Fixture database still open"));
      });
    }
  }
});

async function retainFixture(left: Database, right: Database) {
  return retainCandidate(
    left,
    right,
    {
      version: 1,
      namespace: "fixture namespace",
      loginState: "fixture login",
      clientId: "fixture client",
      id: crypto.randomUUID(),
      local: encodeRevision(await left.historyRevision()),
      remote: encodeRevision(await right.historyRevision()),
      operation: crypto.randomUUID(),
      committed: false,
    },
    () => true
  );
}

async function withVault<T>(
  db: Database,
  id: string,
  use: (vault: NonNullable<Awaited<ReturnType<typeof unlockEncryptedVault>>>) => Promise<T>
) {
  const client = db.client();
  try {
    const vault = await unlockEncryptedVault(localDatabase(client), "fixture password", id);
    if (!vault) throw new Error("Expected a Vault");
    try {
      return await use(vault);
    } finally {
      await vault.close();
    }
  } finally {
    client.close();
  }
}

it.each(["folder deletion", "split envelope conflict"])(
  "stages encrypted history safely after concurrent %s",
  async (scenario) => {
    const left = await Database.inMemory();
    const client = left.client();
    try {
      await client.migrate(MIGRATIONS);
      const original = await createEncryptedVault(
        localDatabase(client),
        "fixture password",
        "Vault"
      );
      const vaultId = original.id;
      const folder = await original.createFolder("Folder");
      const date = "2026-01-02T03:04:05.000Z";
      const note: Secret = {
        id: crypto.randomUUID(),
        kind: "secureNote",
        type: 0,
        name: "Note",
        notes: "Original",
        createdAt: date,
        updatedAt: date,
        favorite: false,
        fields: [],
        history: [],
        folderId: folder.id,
      };
      const conflict = scenario === "split envelope conflict";
      if (conflict) {
        await original.mutateItems({ upsert: [note] });
        await original.saveLockSettings({ timeoutMinutes: 5 });
      }
      await original.close();
      const right = await left.stageHistory(left, vaultHistoryRecords);
      try {
        await withVault(left, vaultId, async (vault) => {
          await vault.mutateItems({ upsert: [{ ...note, notes: "Left edit" }] });
          if (conflict) {
            await vault.renameFolder(folder.id, "Left folder");
            await vault.saveLockSettings({ timeoutMinutes: 1 });
          }
        });
        if (conflict) {
          await withVault(right, vaultId, async (vault) => {
            await vault.mutateItems({ upsert: [{ ...note, notes: "Right edit" }] });
            await vault.renameFolder(folder.id, "Right folder");
            await vault.saveLockSettings({ timeoutMinutes: 15 });
          });
        } else {
          const remote = right.client();
          try {
            await remote.executeRaw("DELETE FROM folders WHERE id = ?", [dbParam(folder.id)]);
          } finally {
            remote.close();
          }
        }
        const revisions = await Promise.all([left.historyRevision(), right.historyRevision()]);
        const candidates: string[] = [];
        for (const [first, second] of [
          [left, right],
          [right, left],
        ] as const) {
          const pending = await retainFixture(first, second);
          try {
            const reopened = await reopenCandidate(pending);
            try {
              candidates.push(pending.candidate);
              await withVault(reopened, vaultId, async (vault) => {
                const { items, folders } = await vault.readItems();
                expect(items).toHaveLength(1);
                const item = items[0];
                if (!item) throw new Error("Missing merged note");
                if (conflict) {
                  expect(["Left edit", "Right edit"]).toContain(item.notes);
                  expect(item).toEqual({ ...note, notes: item.notes });
                  expect(folders).toHaveLength(1);
                  expect(folders[0]?.id).toBe(folder.id);
                  expect(["Left folder", "Right folder"]).toContain(folders[0]?.name);
                  expect([1, 15]).toContain((await vault.loadLockSettings()).timeoutMinutes);
                } else {
                  expect(item).toEqual({ ...note, notes: "Left edit", folderId: null });
                  expect(folders).toEqual([]);
                }
                expect(typeof (await vault.exportBackup())).toBe("string");
                await vault.mutateItems({ upsert: [{ ...item, notes: "Edited again" }] });
                expect((await vault.readItems()).items[0]?.notes).toBe("Edited again");
              });
            } finally {
              reopened.close();
            }
          } finally {
            clearMerge(pending);
          }
        }
        expect(candidates[0]).toBe(candidates[1]);
        expect(await Promise.all([left.historyRevision(), right.historyRevision()])).toEqual(
          revisions
        );
      } finally {
        right.close();
      }
    } finally {
      client.close();
      left.close();
    }
  },
  15000
);
