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
];
