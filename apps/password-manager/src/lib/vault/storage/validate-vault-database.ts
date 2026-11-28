import { type Client, Database, type Migration } from "@ofsdk/sql";
import { vaultHistoryRecords } from "../history-records";
import { fromBase64 } from "./crypto";
import { MIGRATIONS, parseInteger, parseText, parseVaultRow, sharedEncryptedRows } from "./schema";

export const ledgerlessVaultRecords = vaultHistoryRecords.filter(
  ({ table }) => table !== "_ofdb_migrations"
);

export async function hasVaultMigrationLedger(database: Database): Promise<boolean> {
  const client = database.client();
  try {
    return (
      (
        await client.execute(
          "SELECT name FROM tables WHERE name = '_ofdb_migrations'",
          (row) => row
        )
      ).length !== 0
    );
  } finally {
    client.close();
  }
}

function checkEnvelope({ ciphertext, nonce }: { ciphertext: string; nonce: string }) {
  if (fromBase64(ciphertext).byteLength < 16 || fromBase64(nonce).byteLength !== 12)
    throw new Error("Invalid Password Manager encryption envelope.");
}

export async function validateVaultRows(
  database: Database,
  requireVault: boolean
): Promise<string[]> {
  const client = database.client();
  try {
    // Use the selector and shared item readers' projections and parsers, including on empty tables.
    const vaults = await client.execute(
      "SELECT id, name, salt, iv, created_at, authentication, status, migrated FROM vaults",
      (row) => {
        const vault = parseVaultRow(row);
        const migrated = parseInteger(row, "migrated");
        if (
          parseText(row, "status") !== "ready" ||
          (migrated !== 0 && migrated !== 1) ||
          fromBase64(vault.salt).byteLength !== 16 ||
          fromBase64(vault.iv).byteLength !== 12
        )
          throw new Error("Invalid Password Manager vault metadata.");
        if (vault.authentication != null) {
          const proof = JSON.parse(vault.authentication);
          if (
            proof?.version !== 1 ||
            typeof proof.ciphertext !== "string" ||
            typeof proof.nonce !== "string"
          )
            throw new Error("Invalid Password Manager vault authentication envelope.");
          checkEnvelope(proof);
        }
        return vault.id;
      }
    );
    if (requireVault && vaults.length === 0)
      throw new Error("The older Resource does not contain a recognizable Password Manager vault.");
    const ids = new Set(vaults);
    const checkRow = (row: { vaultId: string; ciphertext: string; nonce: string }) => {
      if (!ids.has(row.vaultId)) throw new Error("Encrypted row has no Password Manager vault.");
      checkEnvelope(row);
    };
    for (const table of ["secrets", "folders"]) {
      await client.execute(
        `SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM ${table}`,
        (row) => checkRow(sharedEncryptedRows.parseItemRow(row))
      );
    }
    await client.execute(
      "SELECT vault_id, ciphertext, nonce, updated_at FROM lock_settings",
      (row) => checkRow(sharedEncryptedRows.parseLockSettingsRow(row))
    );
    return vaults;
  } finally {
    client.close();
  }
}

// The per-vault copy does not merge history (its metadata key is an integer).
// Check its active catalog and migration allowlist in the same SQL snapshot as the rows.
export async function requireVaultLayout(
  reader: Pick<Client, "execute">,
  migrations: readonly Migration[]
) {
  const reference = await Database.inMemory();
  const client = reference.client();
  async function layout(reader: Pick<Client, "execute">) {
    const tables = await reader.execute(
      "SELECT name FROM tables ORDER BY name",
      ({ name }) => name
    );
    const fields = await reader.execute(
      "SELECT * FROM table_fields ORDER BY table, position",
      ({ table, name, type, default: defaultValue, position, primary_key }) => [
        table,
        name,
        type,
        defaultValue,
        position,
        primary_key,
      ]
    );
    const ledger = await reader.execute(
      "SELECT version, description, fingerprint, execution_time FROM _ofdb_migrations ORDER BY version",
      (row) => {
        if (parseInteger(row, "execution_time") < 0)
          throw new Error("Invalid migration execution time.");
        const { version, description, fingerprint } = row;
        return [version, description, fingerprint];
      }
    );
    return JSON.stringify([tables, fields, ledger]);
  }
  try {
    await client.migrate(migrations);
    if ((await layout(reader)) !== (await layout(client)))
      throw new Error("Unsupported Password Manager schema or migration history.");
  } finally {
    client.close();
    reference.close();
  }
}

export async function validateVaultDatabase(
  database: Database,
  ledger: boolean,
  requireVault: boolean
) {
  const reference = await Database.inMemory();
  const client = reference.client();
  try {
    if (ledger) await client.migrate(MIGRATIONS);
    else for (const { sql } of MIGRATIONS) await client.executeRaw(sql);
    // Compare active snapshot schemas and the supported ledger, without publishing the result.
    const checked = await database.stageHistory(
      reference,
      ledger ? vaultHistoryRecords : ledgerlessVaultRecords
    );
    checked.close();
  } finally {
    client.close();
    reference.close();
  }
  return validateVaultRows(database, requireVault);
}
