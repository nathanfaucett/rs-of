import type { DbValue } from "@ofsdk/sql";
import type { DatabaseHandle } from "@ofsdk/sync-sdk";
import {
  createEncryptedVault,
  unlockEncryptedVault,
  type VaultDatabase,
  type VaultHandle,
} from "./encrypted-vault";
import { MIGRATIONS, parseOptionalText, parseText, syncedEncryptedRows } from "./storage/schema";
import type { VaultIdentity } from "./storage/VaultRegistry";

function syncedDatabase(handle: DatabaseHandle): VaultDatabase {
  return {
    async read(sql, parse, params) {
      const { columns, rows } = await handle.execute(sql, params);
      return rows.map((cells) => {
        const row: Record<string, DbValue> = Object.create(null);
        for (const [index, column] of columns.entries()) {
          const cell = cells[index];
          if (cell === null) row[column] = { type: "null" };
          else if (typeof cell === "string") row[column] = { type: "text", value: cell };
          else if (typeof cell === "number" || typeof cell === "bigint") {
            row[column] = { type: "integer", value: String(cell) };
          } else throw new Error(`Unexpected SQL value for ${column}`);
        }
        return parse(row);
      });
    },
    async write(sql, params) {
      await handle.execute(sql, params);
    },
    async writeBatch(statements) {
      if (statements.length) {
        await handle.execute(
          statements.map(({ sql }) => sql).join("; "),
          statements.flatMap(({ params }) => params)
        );
      }
    },
    encryptedRows: syncedEncryptedRows,
    parseMeta(row) {
      return {
        id: parseText(row, "id"),
        salt: parseText(row, "salt"),
        iv: parseText(row, "iv"),
        createdAt: parseText(row, "created_at"),
        authentication: parseOptionalText(row, "authentication"),
      };
    },
    sync: (options) => handle.sync(options),
    close: () => handle.close(),
  };
}

export async function createSyncVault(
  password: string,
  name: string,
  handle: DatabaseHandle
): Promise<VaultHandle> {
  for (const { sql } of MIGRATIONS) await handle.execute(sql);
  return createEncryptedVault(syncedDatabase(handle), password, name);
}

export function unlockSyncVault(
  password: string,
  id: string,
  handle: DatabaseHandle
): Promise<VaultHandle | null> {
  return unlockEncryptedVault(syncedDatabase(handle), password, id);
}

export async function listSyncVaults(syncHandle: DatabaseHandle): Promise<VaultIdentity[]> {
  const results = await syncHandle.execute(
    `SELECT id, name, status, migrated FROM vaults WHERE status = 'ready' ORDER BY created_at DESC`,
    []
  );
  return results.rows.map((row: unknown[]) => ({
    id: row[0] as string,
    name: row[1] as string,
    status: "ready" as const,
    migrated: Boolean(row[3]),
  }));
}
