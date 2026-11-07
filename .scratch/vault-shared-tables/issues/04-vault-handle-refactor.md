Status: open

## Task

Refactor `VaultHandle` in `vault.ts` to use static table names with `vault_id` parameter and **typed `select()`**.

## Details

**Remove:**

- `tablePrefix()` function
- `prefixed(table)` method
- `databaseName` field (was per-vault DB name)
- All dynamic SQL: `${tablePrefix(vaultId)}secrets` → `secrets`

**Change:**

- Constructor: `new VaultHandle(key, client, meta, vaultId)` — client is shared
- **All queries use `Client.select()` / `Transaction.select()` with Valibot parsers**
- Static table names + `WHERE vault_id = ?` parameters

**Import schemas from `schema.ts`:**

```typescript
import {
  SecretRowSchema,
  FolderRowSchema,
  LockSettingsRowSchema,
} from "./schema";
import { Client, Transaction, Row } from "@ofsdk/sql";
```

**Update methods to use typed `select()`:**

```typescript
// loadSecrets()
async loadSecrets(): Promise<Secret[]> {
  return navigator.locks.request(`password-manager-items-${this.id}`, async () => {
    await this.verifyVaultExists();
    return this.client.select(
      `SELECT id, ciphertext, nonce, created_at, updated_at FROM secrets WHERE vault_id = ?`,
      (row: Row) => SecretRowSchema.parse({
        id: row.id.value,
        ciphertext: row.ciphertext.value,
        nonce: row.nonce.value,
        createdAt: row.created_at.value,
        updatedAt: row.updated_at.value,
      }),
      [{ type: "text", value: this.vaultId }],
    );
  });
}

// loadFolders()
async loadFolders(): Promise<Folder[]> {
  return navigator.locks.request(`password-manager-items-${this.id}`, async () => {
    await this.verifyVaultExists();
    return this.client.select(
      `SELECT id, ciphertext, nonce, created_at, updated_at FROM folders WHERE vault_id = ?`,
      (row: Row) => FolderRowSchema.parse({
        id: row.id.value,
        ciphertext: row.ciphertext.value,
        nonce: row.nonce.value,
        createdAt: row.created_at.value,
        updatedAt: row.updated_at.value,
      }),
      [{ type: "text", value: this.vaultId }],
    );
  });
}

// encryptAndStore() — use transaction for multi-table writes
async encryptAndStore(item: SecretItem): Promise<void> {
  const tx = await this.client.transaction();
  try {
    await tx.execute(
      `INSERT INTO secrets (id, vault_id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)`,
      [{ type: "text", value: item.id }, { type: "text", value: this.vaultId }, ...],
    );
    await tx.commit();
  } catch {
    await tx.rollback();
    throw;
  }
}

// loadLockSettings()
async loadLockSettings(): Promise<LockSettings> {
  return navigator.locks.request(`password-manager-items-${this.id}`, async () => {
    await this.verifyVaultExists();
    const results = await this.client.select(
      `SELECT ciphertext, nonce, updated_at FROM lock_settings WHERE vault_id = ?`,
      (row: Row) => LockSettingsRowSchema.parse({
        ciphertext: row.ciphertext.value,
        nonce: row.nonce.value,
        updatedAt: row.updated_at.value,
      }),
      [{ type: "text", value: this.vaultId }],
    );
    if (!results[0]) return DEFAULT_LOCK_SETTINGS;
    return decryptLockSettings(this.activeKey(), this.vaultId, results[0]);
  });
}

// saveLockSettings()
async saveLockSettings(settings: LockSettings): Promise<LockSettings> {
  return navigator.locks.request(`password-manager-items-${this.id}`, async () => {
    await this.verifyVaultExists();
    const encrypted = await encryptLockSettings(this.activeKey(), this.vaultId, settings);
    await this.client.execute(
      `INSERT INTO lock_settings (vault_id, ciphertext, nonce, updated_at) VALUES (?, ?, ?, ?)
       ON CONFLICT(vault_id) DO UPDATE SET ciphertext = excluded.ciphertext, nonce = excluded.nonce, updated_at = excluded.updated_at`,
      [{ type: "text", value: this.vaultId }, { type: "text", value: encrypted.ciphertext }, { type: "text", value: encrypted.nonce }, { type: "text", value: new Date().toISOString() }],
    );
    return settings;
  });
}
```

**Update `createVault()`:**

```typescript
export async function createVault(
  password: string,
  name: string,
): Promise<VaultHandle> {
  const vaultId = crypto.randomUUID();
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveKey(password, salt.buffer);
  const meta = {
    id: vaultId,
    salt: toBase64(salt),
    iv: toBase64(iv),
    createdAt: new Date().toISOString(),
  };
  const authentication = await createAuthentication(key, meta);

  const client = SharedDb.getInstance().getClient();

  // Single transaction
  await client.transaction(async (tx) => {
    await tx.execute(
      `INSERT INTO vaults (id, name, salt, iv, created_at, authentication, status, migrated) VALUES (?, ?, ?, ?, ?, ?, 'ready', 1)`,
      [vaultId, name, meta.salt, meta.iv, meta.createdAt, authentication],
    );
  });

  return VaultHandle.create(key, client, meta, vaultId);
}
```

**Update `unlockVault()`:**

```typescript
export async function unlockVault(
  password: string,
  id: string,
): Promise<VaultHandle> {
  const client = SharedDb.getInstance().getClient();

  // Read from vaults table using typed select
  const results = await client.select(
    `SELECT id, salt, iv, created_at, authentication FROM vaults WHERE id = ?`,
    (row: Row) => ({
      id: row.id.value,
      salt: row.salt.value,
      iv: row.iv.value,
      createdAt: row.created_at.value,
      authentication: row.authentication?.value ?? null,
    }),
    [{ type: "text", value: id }],
  );

  if (!results[0]) throw new Error("Vault not found");
  // ... verify authentication, derive key, return VaultHandle
}
```

## Acceptance Criteria

- [ ] No dynamic table names in `VaultHandle`
- [ ] All queries use typed `select()` with Valibot parsers
- [ ] Static table names + `vault_id` parameter
- [ ] `createVault` uses single transaction
- [ ] `unlockVault` reads from `vaults` table with typed `select()`
- [ ] Lock settings use `lock_settings` table with typed `select()`
- [ ] No legacy DB migration logic in `unlockVault`

## Dependencies

Blocked by: 01, 02, 03
