import type { HistoryRecord } from "@ofsdk/sql";

const encryptedFields = {
  envelope: ["ciphertext", "nonce", "updated_at"],
  resolveBy: ["ciphertext", "nonce"],
};
const encryptedItem = {
  identity: "id",
  stable: ["vault_id", "created_at"],
  ...encryptedFields,
};
export const vaultHistoryRecords: HistoryRecord[] = [
  { table: "folders", ...encryptedItem },
  { table: "secrets", ...encryptedItem },
  { table: "lock_settings", identity: "vault_id", ...encryptedFields },
  {
    table: "vaults",
    identity: "id",
    stable: ["salt", "iv", "created_at", "authentication", "status", "migrated"],
    envelope: ["salt", "iv", "created_at", "authentication"],
  },
  {
    table: "_ofdb_migrations",
    identity: "id",
    envelope: ["version", "description", "fingerprint", "execution_time"],
  },
];
