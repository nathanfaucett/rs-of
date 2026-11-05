# Password manager: local encrypted SQL vaults

The app uses the browser `@ofsdk/sql` database. Vault records are encrypted with
AES-256-GCM using a key derived from the master password with PBKDF2-SHA-512
(600,000 iterations). Core vault operations do not require an IdP account or a
Storage service. Use test secrets only; this is a development app, not a qualified
production password manager.

## Local setup

From the workspace root, install dependencies and build the linked SQL package:

```sh
pnpm install --frozen-lockfile
pnpm --dir ../ofdb --filter @ofsdk/sql run build:all
pnpm --filter @ofsdk/password-manager dev
```

The current dependency uses the sibling `ofdb` checkout. The browser must allow
IndexedDB and Web Crypto. Local development must use a secure browser context.

## Schema migrations

`src/lib/vault/migrations.ts` is the complete ordered SQL migration list.
`createVault()` and `unlockVault()` call `Client.migrate()` before accessing vault
metadata or secret records. Database open alone does not run migrations.

Version 1 creates:

- `vault_meta`: a fixed metadata key (`id = 1`), the vault ID, derivation salt,
  initialization vector, and creation timestamp. Duplicate creation fails instead
  of adding another metadata row or replacing the original salt.
- `secrets`: record ID, ciphertext, per-record nonce, and timestamps. Secret
  contents remain encrypted; no plaintext secret index is added.

The SQL runner owns `_ofdb_migrations`. Each migration and its history record
commit atomically. Reopening validates history and skips completed migrations.
Changed migration contents, unknown applied versions, and malformed history fail
without reset or repair. Writes use named columns and bound parameters so future
column additions do not depend on positional inserts or hand-built SQL literals.

### Adding a migration

Append the next contiguous version to `vaultMigrations`. Never edit an applied
migration, including its whitespace. Use only SQL supported by `ofdb`; current
ALTER support is limited to `ADD COLUMN`. Update queries and tests with the
schema change. Do not add another schema-version table or migration runner.

A failed commit can have an unknown durable outcome. Do not replay its SQL or
edit migration history. A later explicit create/unlock attempt checks history
again. Earlier migrations remain committed if a later migration fails.

### Development reset

Old development databases have no supported upgrade path. They are not adopted,
repaired, or deleted automatically. Clear their IndexedDB database explicitly in
browser developer tools, then create a new vault. This permanently deletes their
contents. The database name starts with `ofdb:sql:password-manager-vault-`.

## Collection behavior and limits

The local collection retains the official TanStack binding, validates decrypted
records, and reloads the complete vault after writes. Schema migrations do not
change that full-vault reload cost. Secret payload validation and SQL schema
migration are separate concerns; the SQL runner does not decrypt records.

Existing development limits remain:

- Display names determine database names after normalization; distinct display
  names can collide. Duplicate creation now fails instead of adding metadata.
- An empty vault does not authenticate its master password against an independent
  verifier. Populated vaults check a stored record. This needs separate work.
- The current encrypted export is not a self-contained backup or restore format.
- Deletion is not secure erasure. SQL history, browser storage, and backups can
  retain prior ciphertext. No JavaScript memory-zeroization guarantee is made.

Do not log passwords, keys, decrypted records, or SQL parameters.

## Checks

```sh
pnpm --filter @ofsdk/password-manager test
pnpm --filter @ofsdk/password-manager build
pnpm --filter @ofsdk/password-manager check
```

Vault tests use real browser SQL, migrations, IndexedDB, and Web Crypto. They cover
fresh/prepared databases, repeated startup, old/newer schema rejection, duplicate
creation, encrypted CRUD, and lock/unlock persistence. Collection and editor tests
cover the existing local workflow. These checks do not qualify production vault
security, secure erasure, backup recovery, or large-vault performance.
