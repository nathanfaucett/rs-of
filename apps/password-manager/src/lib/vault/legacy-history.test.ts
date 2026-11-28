import { Database } from "@ofsdk/sql";
import { expect, it, vi } from "vitest";
import { legacySyncDDL } from "../../test/legacy-sync-ddl";
import { createEncryptedVault } from "./encrypted-vault";
import { stageLegacyUpgrade } from "./legacy-history";
import { encodeRevision, readPendingMerge, retainCandidate } from "./merge-history";
import { MIGRATIONS } from "./storage/schema";
import { localDatabase } from "./vault";

async function createFixture(db: Database, sql?: readonly string[]) {
  const client = db.client();
  try {
    if (sql) for (const statement of sql) await client.executeRaw(statement);
    else await client.migrate(MIGRATIONS);
    const vault = await createEncryptedVault(
      localDatabase(client),
      "fixture password",
      "Baseline vault"
    );
    await vault.close();
  } finally {
    client.close();
  }
}

it("rejects the wrong declared column type even with a readable vault and no items", async () => {
  const local = await Database.inMemory();
  const source = await Database.inMemory();
  try {
    await createFixture(
      source,
      legacySyncDDL.map((sql) => sql.replace("nonce TEXT", "nonce BLOB"))
    );
    await expect(stageLegacyUpgrade(local, source, true)).rejects.toThrow(
      "Incompatible history table schemas"
    );
    await expect(local.initialRestoreIsEligible()).resolves.toBe(true);
  } finally {
    source.close();
    local.close();
  }
});

it("rejects an unknown ledger instead of treating it as the original ledger-free schema", async () => {
  const local = await Database.inMemory();
  const source = await Database.inMemory();
  const client = source.client();
  try {
    await createFixture(source, legacySyncDDL);
    await client.migrate([
      ...MIGRATIONS,
      { version: 5, description: "Unrecognized", sql: MIGRATIONS[0].sql },
    ]);
    const before = await source.historyRevision();
    await expect(stageLegacyUpgrade(local, source, true)).rejects.toThrow(
      "Incompatible migration history"
    );
    expect(await source.historyRevision()).toEqual(before);
  } finally {
    client.close();
    source.close();
    local.close();
  }
});

it("validates the final selector rows before retiring a source or retaining a candidate", async () => {
  const local = await Database.inMemory();
  const source = await Database.inMemory();
  const client = local.client();
  const retire = vi.fn(async () => {});
  try {
    await createFixture(source, legacySyncDDL);
    await createFixture(local);
    await client.executeRaw("UPDATE vaults SET name = ''");
    const before = await local.historyRevision();
    await expect(
      retainCandidate(
        local,
        source,
        {
          version: 1,
          id: crypto.randomUUID(),
          local: encodeRevision(before),
          remote: encodeRevision(await source.historyRevision()),
          legacy: { explicit: true, browser: encodeRevision(await source.historyRevision()) },
          operation: crypto.randomUUID(),
          namespace: "fixture namespace",
          loginState: "fixture login",
          clientId: "fixture client",
          committed: false,
          attempted: false,
        },
        () => true,
        { database: source, retire, revision: "1", close: () => {} }
      )
    ).rejects.toThrow("SQL row parser failed");
    expect(retire).not.toHaveBeenCalled();
    expect(readPendingMerge()).toBeUndefined();
    expect(await local.historyRevision()).toEqual(before);
  } finally {
    client.close();
    source.close();
    local.close();
  }
});
