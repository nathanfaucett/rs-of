// Synced creation DDL from the pre-unification app (5b69a3e5). It never wrote a ledger.
const CREATE_VAULTS_TABLE = `
CREATE TABLE IF NOT EXISTS vaults (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  salt TEXT NOT NULL,        -- base64, 16 bytes
  iv TEXT NOT NULL,          -- base64, 12 bytes
  created_at TEXT NOT NULL,
  authentication TEXT,       -- JSON { version, ciphertext, nonce }
  status TEXT NOT NULL DEFAULT 'ready',  -- 'ready' only (no 'pending')
  migrated INTEGER NOT NULL DEFAULT 1    -- legacy migration flag
)
`;

const CREATE_LOCK_SETTINGS_TABLE = `
CREATE TABLE IF NOT EXISTS lock_settings (
  vault_id TEXT PRIMARY KEY,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  updated_at TEXT NOT NULL
)
`;

const CREATE_SECRETS_TABLE = `
CREATE TABLE IF NOT EXISTS secrets (
  id TEXT PRIMARY KEY,
  vault_id TEXT NOT NULL,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_secrets_vault ON secrets(vault_id)
`;

const CREATE_FOLDERS_TABLE = `
CREATE TABLE IF NOT EXISTS folders (
  id TEXT PRIMARY KEY,
  vault_id TEXT NOT NULL,
  ciphertext TEXT NOT NULL,
  nonce TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_folders_vault ON folders(vault_id)
`;

export const legacySyncDDL = [
  CREATE_VAULTS_TABLE,
  CREATE_LOCK_SETTINGS_TABLE,
  CREATE_SECRETS_TABLE,
  CREATE_FOLDERS_TABLE,
];
