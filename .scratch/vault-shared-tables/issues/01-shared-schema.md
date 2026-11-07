Status: open

## Task

Create shared table schema and migration list in `src/lib/vault/schema.ts`.

## Details

Define:

1. `CREATE_VAULTS_TABLE` — vault registry (replaces IndexedDB registry)
2. `CREATE_LOCK_SETTINGS_TABLE` — per-vault lock settings
3. `CREATE_SECRETS_TABLE` — secrets with `vault_id` FK
4. `CREATE_FOLDERS_TABLE` — folders with `vault_id` FK
5. `MIGRATIONS` array — ordered list for `client.migrate()`

All tables in single shared DB (`password-manager-v1`).

**Include Valibot schema exports** for typed `select()` parsers:

- `VaultRowSchema` — maps `vaults` table row
- `SecretRowSchema` — maps `secrets` table row
- `FolderRowSchema` — maps `folders` table row
- `LockSettingsRowSchema` — maps `lock_settings` table row

## Acceptance Criteria

- [ ] Schema file exports all DDL strings
- [ ] Migrations apply cleanly to empty DB
- [ ] Foreign keys with `ON DELETE CASCADE` for vault cleanup
- [ ] Indexes on `vault_id` for query performance
- [ ] No `pending` status — only `ready`
- [ ] Valibot schemas exported for parser reuse

## Notes

Reference: Bitwarden `ciphers` table with `user_uuid`/`organization_uuid` ownership.
Typed `select()` API: `client.select(sql, parser, params)` from `@ofsdk/sql`.
