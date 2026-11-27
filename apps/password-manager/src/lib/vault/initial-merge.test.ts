import { Database } from "@ofsdk/sql";
import { expect, it } from "vitest";
import { createEncryptedVault, unlockEncryptedVault } from "./encrypted-vault";
import {
  clearMerge,
  encodeRevision,
  readPendingMerge,
  reopenCandidate,
  retainCandidate,
} from "./merge-history";
import { MIGRATIONS, parseText } from "./storage/schema";
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
    const pending = await retainCandidate(
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
