import type { Migration } from "@ofsdk/sql";

export const vaultMigrations: readonly Migration[] = [
    {
        version: 1,
        sql: `CREATE TABLE vault_meta (
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
    },
    {
        version: 2,
        sql: "ALTER TABLE vault_meta ADD COLUMN authentication TEXT",
    },
    {
        version: 3,
        sql: "ALTER TABLE vault_meta ADD COLUMN item_format INTEGER NOT NULL DEFAULT 1; ALTER TABLE vault_meta ADD COLUMN transition_backup TEXT",
    },
    {
        version: 4,
        sql: "ALTER TABLE vault_meta ADD COLUMN lock_settings TEXT",
    },
];
