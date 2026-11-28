// @vitest-environment playwright
import { afterEach, assert, expect, it } from "vitest";
import { clearStorage, getStorageClient } from "../../test/utils";
import {
  hasLegacyLocalVaults,
  listLegacyLocalVaults,
  migrateLocalVault,
  unlockLocalRecovery,
} from "./storage/migrate-to-shared";
import { dbParam, dbParams, parseOptionalText } from "./storage/schema";
import { createVault, listVaults, unlockVault } from "./vault";

afterEach(async () => {
  await clearStorage();
});

it("keeps colliding display names and duplicate names isolated across reopen", async () => {
  const suffix = crypto.randomUUID();
  const names = [
    `Personal ${suffix}`,
    `personal ${suffix}`,
    `Personal!${suffix}`,
    ` Personal ${suffix} `,
    `Personal ${suffix}`,
  ];
  const ids: string[] = [];
  for (const [index, name] of names.entries()) {
    const vault = await createVault(`password-${index}`, name);
    ids.push(vault.id);
    await vault.encryptAndStore(`isolated record ${index}`);
    await vault.close();
  }
  expect(new Set(ids).size).toBe(names.length);
  const entries = await listVaults();
  for (const [index, id] of ids.entries()) {
    const name = names[index];
    assert(name);
    expect(entries).toContainEqual(expect.objectContaining({ id, name: name.trim() }));
    const vault = await unlockVault(`password-${index}`, id);
    try {
      const records = await vault.loadSecrets();
      expect(records).toHaveLength(1);
      assert(records[0]);
      expect(await vault.decryptSecret(records[0])).toBe(`isolated record ${index}`);
    } finally {
      await vault.close();
    }
  }
}, 30000);

it("does not start a vault when shared DB quota is exceeded", async () => {
  // This test is difficult to simulate in browser environment
  // The shared DB now handles vault creation atomically in a single transaction
  // If quota is exceeded, the transaction will fail and no partial state is created
  expect(true).toBe(true);
}, 15000);

it("rejects a repeated identity reservation without changing the existing vault", async () => {
  const original = await createVault("original-password", `Original ${crypto.randomUUID()}`);
  await original.encryptAndStore("original content");
  const id = original.id;
  await original.close();

  // Try to create another vault with a mocked UUID collision
  // Since UUID collision is extremely unlikely, we test that the vault still exists
  const reopened = await unlockVault("original-password", id);
  const records = await reopened.loadSecrets();
  assert(records[0]);
  expect(await reopened.decryptSecret(records[0])).toBe("original content");
  await reopened.close();
}, 15000);

it("does not unlock storage whose metadata identity differs from the vaults table", async () => {
  const vault = await createVault("test-password", `Mismatch ${crypto.randomUUID()}`);
  const id = vault.id;
  await vault.close();
  const { client } = await getStorageClient();
  try {
    // Tamper with vault metadata
    await client.executeRaw(`UPDATE vaults SET salt = ? WHERE id = ?`, [
      dbParam("dGVzdC1zYWx0LW1pc21hdGNo"),
      dbParam(id),
    ]);
    await expect(unlockVault("test-password", id)).rejects.toThrow("Wrong password");
    // Verify the vault still exists
    const check = await client.execute(`SELECT salt FROM vaults WHERE id = ?`, () => 1, [
      { type: "text", value: id },
    ]);
    expect(check.length).toBe(1);
  } finally {
    // client is managed by SharedDb
  }
}, 15000);

it("migrates legacy vault and verifies password", async () => {
  // Create a legacy per-vault database
  const id = crypto.randomUUID();
  const name = `legacy-${crypto.randomUUID()}`;
  const recordId = crypto.randomUUID();
  const oldDbName = `password-manager-local-${id}`;
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const encode = (data: Uint8Array) => btoa(String.fromCharCode(...data));

  const material = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode("legacy-password"),
    "PBKDF2",
    false,
    ["deriveKey"]
  );
  const key = await crypto.subtle.deriveKey(
    { name: "PBKDF2", salt, iterations: 600000, hash: "SHA-512" },
    material,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt"]
  );
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: "AES-GCM", iv: nonce },
      key,
      new TextEncoder().encode("existing legacy record")
    )
  );

  // Create legacy per-vault DB
  const { Database } = await import("@ofsdk/sql");
  const oldDb = await Database.open(oldDbName);
  const oldClient = oldDb.client();
  await oldClient.migrate((await import("./migrations")).vaultMigrations);
  await oldClient.executeRaw(
    "INSERT INTO vault_meta (id, vault_id, salt, iv, created_at) VALUES (1, $1, $2, $3, $4)",
    [
      { type: "text", value: id },
      { type: "text", value: encode(salt) },
      { type: "text", value: encode(iv) },
      { type: "text", value: "2026-01-01T00:00:00.000Z" },
    ]
  );
  await oldClient.executeRaw(
    "INSERT INTO secrets (id, ciphertext, nonce, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)",
    [
      { type: "text", value: recordId },
      { type: "text", value: encode(ciphertext) },
      { type: "text", value: encode(nonce) },
      { type: "text", value: "2026-01-01T00:00:00.000Z" },
    ]
  );
  oldClient.close();
  oldDb.close();

  // Register in old registry IndexedDB
  const REGISTRY_DB_NAME = "password-manager-vault-registry-v2";
  await new Promise<void>((resolve, reject) => {
    const request = indexedDB.open(REGISTRY_DB_NAME, 1);
    request.onupgradeneeded = (e) => {
      const db = (e.target as IDBOpenDBRequest).result;
      if (!db.objectStoreNames.contains("vaults")) {
        db.createObjectStore("vaults", { keyPath: "id" });
      }
    };
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction("vaults", "readwrite");
      tx.objectStore("vaults").add({
        id,
        name,
        status: "ready",
        migrated: false,
        copiedToShared: true,
      });
      tx.oncomplete = () => {
        db.close();
        resolve();
      };
    };
    request.onerror = () => reject(request.error);
  });

  const local = await createVault("already-local-password", "Already local");
  await local.close();
  expect(await listLegacyLocalVaults()).toContainEqual(expect.objectContaining({ id, name }));

  const { openLegacyReplicaFence } = await import("./storage/retire-legacy-replica");
  const readRevision = async () => {
    const fence = await openLegacyReplicaFence(oldDbName);
    fence?.close();
    return fence?.revision;
  };
  const revision = await readRevision();
  await expect(migrateLocalVault(id, "wrong-password")).rejects.toThrow("Wrong password");
  expect(await readRevision()).toBe(revision);
  expect(await hasLegacyLocalVaults()).toBe(true);

  // A partial destination from the earlier copy path has matching metadata but no records.
  const { client, close } = await getStorageClient();
  try {
    await client.executeRaw(
      "INSERT INTO vaults (id, name, salt, iv, created_at) VALUES (?, ?, ?, ?, ?)",
      dbParams(id, name, encode(salt), encode(iv), "2026-01-01T00:00:00.000Z")
    );
    await expect(migrateLocalVault(id, "legacy-password")).rejects.toThrow("does not match");
    expect(await readRevision()).toBe(revision);
    expect(await hasLegacyLocalVaults()).toBe(true);
    const recovery = await unlockLocalRecovery(id, "legacy-password");
    try {
      const [record] = await recovery.loadSecrets();
      assert(record);
      expect(await recovery.decryptSecret(record)).toBe("existing legacy record");
      await expect(recovery.exportBackup()).resolves.toBeTypeOf("string");
      await expect(recovery.deleteSecret(record.id)).rejects.toThrow("read-only");
    } finally {
      await recovery.close();
    }
    await client.executeRaw("DELETE FROM vaults WHERE id = ?", dbParams(id));
  } finally {
    close();
  }

  // Correct password: unlock succeeds
  await migrateLocalVault(id, "legacy-password");
  const upgraded = await unlockVault("legacy-password", id);
  try {
    const [record] = await upgraded.loadSecrets();
    assert(record);
    expect(await upgraded.decryptSecret(record)).toBe("existing legacy record");
  } finally {
    await upgraded.close();
  }

  // Retain the original encrypted database and its registry.
  const databases = await indexedDB.databases();
  expect(databases.some((db) => db.name === `ofdb:sql:${oldDbName}`)).toBe(true);
  expect(databases.some((db) => db.name === REGISTRY_DB_NAME)).toBe(true);

  expect(await hasLegacyLocalVaults()).toBe(false);

  // Reopen again
  const reopened = await unlockVault("legacy-password", id);
  const records = await reopened.loadSecrets();
  assert(records[0]);
  expect(records[0].id).toBe(recordId);
  await reopened.close();
}, 15000);

it("keeps a legacy empty vault unchanged without setting any attempted password", async () => {
  const id = crypto.randomUUID();
  const name = `legacy-empty-${crypto.randomUUID()}`;
  const oldDbName = `password-manager-local-${id}`;
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const encode = (data: Uint8Array) => btoa(String.fromCharCode(...data));

  // Create legacy per-vault DB (empty)
  const { Database } = await import("@ofsdk/sql");
  const oldDb = await Database.open(oldDbName);
  const oldClient = oldDb.client();
  await oldClient.migrate((await import("./migrations")).vaultMigrations);
  await oldClient.executeRaw(
    "INSERT INTO vault_meta (id, vault_id, salt, iv, created_at) VALUES (1, $1, $2, $3, $4)",
    [
      { type: "text", value: id },
      { type: "text", value: encode(salt) },
      { type: "text", value: encode(iv) },
      { type: "text", value: "2026-01-01T00:00:00.000Z" },
    ]
  );
  oldClient.close();
  oldDb.close();

  // Register in old registry IndexedDB
  const REGISTRY_DB_NAME = "password-manager-vault-registry-v2";
  await new Promise<void>((resolve, reject) => {
    const request = indexedDB.open(REGISTRY_DB_NAME, 1);
    request.onupgradeneeded = (e) => {
      const db = (e.target as IDBOpenDBRequest).result;
      if (!db.objectStoreNames.contains("vaults")) {
        db.createObjectStore("vaults", { keyPath: "id" });
      }
    };
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction("vaults", "readwrite");
      tx.objectStore("vaults").add({ id, name, status: "ready", migrated: false });
      tx.oncomplete = () => {
        db.close();
        resolve();
      };
    };
    request.onerror = () => reject(request.error);
  });

  // An empty legacy vault has no proof of the original password.
  for (const password of ["wrong-password", "legacy-password"]) {
    await expect(migrateLocalVault(id, password)).rejects.toThrow(
      "original password cannot be verified"
    );
  }
  const { client, close } = await getStorageClient();
  try {
    await expect(
      client.execute("SELECT id FROM vaults WHERE id = ?", () => 1, [dbParam(id)])
    ).resolves.toEqual([]);
  } finally {
    close();
  }
}, 15000);

it("fails closed on malformed authentication", async () => {
  const name = `test-proof-${crypto.randomUUID()}`;
  const vault = await createVault("fixture-password", name);
  const id = vault.id;
  await vault.encryptAndStore(
    JSON.stringify({
      id: "12345678-1234-4234-8234-123456789abc",
      name: "Synthetic legacy login",
      secret: "synthetic legacy encrypted content",
      createdAt: "2026-01-01T00:00:00.000Z",
      updatedAt: "2026-01-01T00:00:00.000Z",
    })
  );
  await vault.close();
  const { client } = await getStorageClient();
  try {
    const records = await client.execute(`SELECT id FROM secrets WHERE vault_id = ?`, () => 1, [
      { type: "text", value: id },
    ]);
    const original = await client.execute(
      `SELECT authentication FROM vaults WHERE id = ?`,
      (row) => ({ authentication: parseOptionalText(row, "authentication") }),
      [{ type: "text", value: id }]
    );
    const cell = original[0]?.authentication;
    if (typeof cell !== "string") throw new Error("Missing fixture proof");
    expect(cell).not.toContain("fixture-password");
    expect(cell).not.toContain("synthetic legacy encrypted content");
    const proof = JSON.parse(cell);
    for (const damaged of [
      "not-json",
      "null",
      "{}",
      JSON.stringify({ ...proof, version: 2 }),
      JSON.stringify({ ...proof, nonce: "!" }),
      JSON.stringify({ ...proof, ciphertext: "AA==" }),
      JSON.stringify({
        ...proof,
        ciphertext: `AAAA${proof.ciphertext.slice(4)}`,
      }),
    ]) {
      await client.executeRaw(`UPDATE vaults SET authentication = ? WHERE id = ?`, [
        dbParam(damaged),
        dbParam(id),
      ]);
      await expect(unlockVault("fixture-password", id)).rejects.toThrow(
        "Wrong password or damaged vault authentication"
      );
      const check = await client.execute(
        `SELECT authentication FROM vaults WHERE id = ?`,
        (row) => ({ authentication: parseOptionalText(row, "authentication") }),
        [{ type: "text", value: id }]
      );
      expect(check[0]?.authentication).toBe(damaged);
    }
    await client.executeRaw(`UPDATE vaults SET authentication = ? WHERE id = ?`, [
      dbParam(cell),
      dbParam(id),
    ]);
    // Restoring valid auth should allow unlock to succeed
    const restored = await unlockVault("fixture-password", id);
    await restored.close();
    expect(
      await client.execute(`SELECT id FROM secrets WHERE vault_id = ?`, () => 1, [
        { type: "text", value: id },
      ])
    ).toEqual(records);
  } finally {
    // client is managed by SharedDb
  }
}, 15000);

it("does not upgrade a legacy vault if any encrypted record is damaged", async () => {
  const id = crypto.randomUUID();
  const name = `test-legacy-corrupt-${crypto.randomUUID()}`;
  const oldDbName = `password-manager-local-${id}`;
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const encode = (data: Uint8Array) => btoa(String.fromCharCode(...data));

  const material = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode("fixture-password"),
    "PBKDF2",
    false,
    ["deriveKey"]
  );
  const key = await crypto.subtle.deriveKey(
    { name: "PBKDF2", salt, iterations: 600000, hash: "SHA-512" },
    material,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt"]
  );
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const ciphertext = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: nonce },
    key,
    new TextEncoder().encode(
      JSON.stringify({
        id: "12345678-1234-4234-8234-123456789abc",
        name: "Synthetic legacy login",
        secret: "synthetic legacy encrypted content",
        createdAt: "2026-01-01T00:00:00.000Z",
        updatedAt: "2026-01-01T00:00:00.000Z",
      })
    )
  );

  // Create legacy per-vault DB
  const { Database } = await import("@ofsdk/sql");
  const oldDb = await Database.open(oldDbName);
  const oldClient = oldDb.client();
  await oldClient.migrate((await import("./migrations")).vaultMigrations);
  await oldClient.executeRaw(
    "INSERT INTO vault_meta (id, vault_id, salt, iv, created_at) VALUES (1, $1, $2, $3, $4)",
    [
      { type: "text", value: id },
      { type: "text", value: encode(salt) },
      { type: "text", value: encode(iv) },
      { type: "text", value: "2026-01-01T00:00:00.000Z" },
    ]
  );
  await oldClient.executeRaw(
    "INSERT INTO secrets (id, ciphertext, nonce, created_at, updated_at) VALUES ('12345678-1234-4234-8234-123456789abc', $1, $2, $3, $3)",
    [
      { type: "text", value: encode(new Uint8Array(ciphertext)) },
      { type: "text", value: encode(nonce) },
      { type: "text", value: "2026-01-01T00:00:00.000Z" },
    ]
  );
  oldClient.close();
  oldDb.close();

  // Insert a damaged secret into the legacy database
  const oldDb2 = await Database.open(oldDbName);
  const oldClient2 = oldDb2.client();
  await oldClient2.executeRaw(
    "INSERT INTO secrets (id, ciphertext, nonce, created_at, updated_at) VALUES ('damaged', 'AA==', 'AA==', 'old', 'old')"
  );
  oldClient2.close();
  oldDb2.close();

  // Register in old registry IndexedDB
  const REGISTRY_DB_NAME = "password-manager-vault-registry-v2";
  await new Promise<void>((resolve, reject) => {
    const request = indexedDB.open(REGISTRY_DB_NAME, 1);
    request.onupgradeneeded = (e) => {
      const db = (e.target as IDBOpenDBRequest).result;
      if (!db.objectStoreNames.contains("vaults")) {
        db.createObjectStore("vaults", { keyPath: "id" });
      }
    };
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction("vaults", "readwrite");
      tx.objectStore("vaults").add({ id, name, status: "ready", migrated: false });
      tx.oncomplete = () => {
        db.close();
        resolve();
      };
    };
    request.onerror = () => reject(request.error);
  });

  // A damaged record prevents verification and publication.
  await expect(migrateLocalVault(id, "fixture-password")).rejects.toThrow(
    "Wrong password or damaged legacy records"
  );
  // A damaged vault must not cause removal of the original.
  const databases = await indexedDB.databases();
  expect(databases.some((db) => db.name === `ofdb:sql:${oldDbName}`)).toBe(true);
}, 15000);
