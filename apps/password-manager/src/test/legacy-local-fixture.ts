import { Database, type DbValue } from "@ofsdk/sql";

// SQL bytes from 2cf67c5b:apps/password-manager/src/lib/vault/migrations.ts.
const migrations = [
  `CREATE TABLE vault_meta (
    id INTEGER PRIMARY KEY,
    vault_id TEXT NOT NULL,
    salt TEXT NOT NULL,
    iv TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE secrets (
    id TEXT PRIMARY KEY,
    ciphertext TEXT NOT NULL,
    nonce TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)`,
  "ALTER TABLE vault_meta ADD COLUMN authentication TEXT",
  "ALTER TABLE vault_meta ADD COLUMN item_format INTEGER NOT NULL DEFAULT 1; ALTER TABLE vault_meta ADD COLUMN transition_backup TEXT",
  "CREATE TABLE folders (id TEXT PRIMARY KEY, ciphertext TEXT NOT NULL, nonce TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
  "ALTER TABLE vault_meta ADD COLUMN lock_settings TEXT",
];

export const legacyPassword = "synthetic-pre-unification-password";
const createdAt = "2026-01-02T03:04:05.000Z";
const updatedAt = "2026-01-03T03:04:05.000Z";
const folder = { id: "30000000-0000-4000-8000-000000000002", name: "Legacy folder" };
const login = {
  id: "30000000-0000-4000-8000-000000000003",
  name: "Legacy Login",
  secret: "Synthetic existing secret",
  createdAt,
  updatedAt,
  deletedAt: updatedAt,
  history: [{ secret: "Synthetic previous secret", changedAt: createdAt }],
};
const base64 = (bytes: Uint8Array) => btoa(String.fromCharCode(...bytes));
const encode = (value: string) => new TextEncoder().encode(value);
const text = (...values: string[]): DbValue[] => values.map((value) => ({ type: "text", value }));

export async function seedLegacyLocal(mode: "new" | "batch" | "transition") {
  const id = crypto.randomUUID();
  const name = "Pre-unification vault";
  const original = `password-manager-local-${id}`;
  const databaseName =
    mode === "new" ? original : `password-manager-${mode}-${crypto.randomUUID()}`;
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const meta = {
    id,
    salt: base64(salt),
    iv: base64(crypto.getRandomValues(new Uint8Array(12))),
    createdAt,
  };
  const material = await crypto.subtle.importKey("raw", encode(legacyPassword), "PBKDF2", false, [
    "deriveKey",
  ]);
  const key = await crypto.subtle.deriveKey(
    { name: "PBKDF2", salt, iterations: 600_000, hash: "SHA-512" },
    material,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt"]
  );
  async function encrypt(value: unknown, purpose?: string) {
    const nonce = crypto.getRandomValues(new Uint8Array(12));
    const ciphertext = await crypto.subtle.encrypt(
      { name: "AES-GCM", iv: nonce, ...(purpose && { additionalData: encode(purpose) }) },
      key,
      encode(JSON.stringify(value))
    );
    return { ciphertext: base64(new Uint8Array(ciphertext)), nonce: base64(nonce) };
  }
  const authentication = JSON.stringify({
    version: 1,
    ...(await encrypt([
      "password-manager/vault-authentication",
      1,
      id,
      meta.salt,
      meta.iv,
      createdAt,
    ])),
  });
  const settings = { timeoutMinutes: 15 };
  const lock = await encrypt(settings, `password-manager/lock-settings/v1/${id}`);
  const lockSettings = JSON.stringify(lock);
  const oldRecord = { id: login.id, ...(await encrypt(login)), createdAt, updatedAt };
  const item = {
    ...login,
    kind: "login",
    favorite: false,
    fields: [],
    uris: [],
    ...(mode === "batch" && { folderId: folder.id }),
  };
  const record = { ...oldRecord, ...(await encrypt(item)) };
  const folderRecord = mode === "batch" ? await encrypt(folder) : null;
  let backup: string | null = null;
  if (mode === "transition") {
    const format = "password-manager/encrypted-backup";
    const kdf = { name: "PBKDF2", hash: "SHA-512", iterations: 600_000, salt: meta.salt };
    const cipher = "AES-256-GCM";
    backup = JSON.stringify({
      format,
      version: 1,
      kdf,
      cipher,
      ...(await encrypt(
        { meta, authentication, name, lockSettings, secrets: [oldRecord] },
        JSON.stringify([format, 1, kdf.name, kdf.hash, kdf.iterations, meta.salt, cipher])
      )),
    });
  }

  async function seed(name: string, active: boolean) {
    const db = await Database.open(name);
    const client = db.client();
    try {
      // The SQL runner used by the old app (rs-ofdb e3e57e8c) stored raw-SQL SHA-256 as text.
      await client.executeRaw(
        "CREATE TABLE _ofdb_migrations (version INTEGER PRIMARY KEY, fingerprint TEXT)"
      );
      for (const [index, sql] of migrations.entries()) {
        const bytes = new Uint8Array(await crypto.subtle.digest("SHA-256", encode(sql)));
        await client.executeRaw(sql);
        await client.executeRaw(
          "INSERT INTO _ofdb_migrations (version, fingerprint) VALUES (?, ?)",
          [
            { type: "integer", value: String(index + 1) },
            ...text(Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("")),
          ]
        );
      }
      const legacyLogin = !active && mode === "transition";
      await client.executeRaw(
        "INSERT INTO vault_meta (id, vault_id, salt, iv, created_at, authentication, item_format, transition_backup, lock_settings) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?)",
        [
          ...text(id, meta.salt, meta.iv, createdAt, authentication),
          { type: "integer", value: legacyLogin ? "1" : "2" },
          active && backup !== null ? { type: "text", value: backup } : { type: "null" },
          ...text(lockSettings),
        ]
      );
      const saved = active
        ? record
        : legacyLogin
          ? oldRecord
          : {
              ...oldRecord,
              ...(await encrypt({ ...item, name: "Stale original", folderId: undefined })),
            };
      await client.executeRaw(
        "INSERT INTO secrets VALUES (?, ?, ?, ?, ?)",
        text(saved.id, saved.ciphertext, saved.nonce, createdAt, updatedAt)
      );
      if (active && folderRecord) {
        await client.executeRaw(
          "INSERT INTO folders VALUES (?, ?, ?, ?, ?)",
          text(folder.id, folderRecord.ciphertext, folderRecord.nonce, createdAt, updatedAt)
        );
      }
    } finally {
      client.close();
      db.close();
    }
  }

  if (databaseName !== original) await seed(original, false);
  await seed(databaseName, true);
  await new Promise<void>((resolve, reject) => {
    const request = indexedDB.open("password-manager-vault-registry-v1", 1);
    request.onupgradeneeded = () => {
      request.result
        .createObjectStore("vaults", { keyPath: "id" })
        .createIndex("databaseName", "databaseName", { unique: true });
    };
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction("vaults", "readwrite");
      tx.objectStore("vaults").add({
        id,
        name,
        databaseName,
        status: "ready",
        ...(databaseName !== original && { previousDatabaseName: original }),
      });
      tx.oncomplete = () => {
        db.close();
        resolve();
      };
      tx.onabort = () => {
        db.close();
        reject(tx.error);
      };
    };
  });
  return {
    id,
    name,
    databaseName,
    original,
    meta,
    authentication,
    lock,
    settings,
    item,
    record,
    folder,
    folderRecord,
    backup,
    oldRecord,
  };
}
