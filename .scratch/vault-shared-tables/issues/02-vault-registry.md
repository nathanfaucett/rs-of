Status: open

## Task

Create `VaultRegistry.ts` to replace `registry.ts` — queries `vaults` table in shared DB using **typed `select()`**.

## Details

Replace IndexedDB registry with SQL queries using `@ofsdk/sql` typed `select()`:

```typescript
import { Client, Row } from "@ofsdk/sql";
import * as v from "valibot";
import { VaultRowSchema } from "./schema";

export interface VaultIdentity {
  id: string;
  name: string;
  status: "ready";
  migrated: boolean;
}

export class VaultRegistry {
  private client: Client;

  constructor(client: Client) {
    this.client = client;
  }

  async listVaultIdentities(): Promise<VaultIdentity[]> {
    return this.client.select(
      `SELECT id, name, status, migrated FROM vaults`,
      (row: Row) =>
        VaultRowSchema.parse({
          id: row.id.value,
          name: row.name.value,
          status: row.status.value as "ready",
          migrated: row.migrated.value === "1",
        }),
    );
  }

  async findVault(id: string): Promise<VaultIdentity> {
    const results = await this.client.select(
      `SELECT id, name, status, migrated FROM vaults WHERE id = ?`,
      (row: Row) =>
        VaultRowSchema.parse({
          id: row.id.value,
          name: row.name.value,
          status: row.status.value as "ready",
          migrated: row.migrated.value === "1",
        }),
      [{ type: "text", value: id }],
    );
    if (!results[0]) throw new Error(`Vault ${id} not found`);
    return results[0];
  }

  async reserveVault(
    name: string,
    salt: string,
    iv: string,
  ): Promise<VaultIdentity> {
    const id = crypto.randomUUID();
    const createdAt = new Date().toISOString();

    await this.client.transaction(async (tx) => {
      await tx.execute(
        `INSERT INTO vaults (id, name, salt, iv, created_at, status, migrated) VALUES (?, ?, ?, ?, ?, 'ready', 1)`,
        [
          { type: "text", value: id },
          { type: "text", value: name },
          { type: "text", value: salt },
          { type: "text", value: iv },
          { type: "text", value: createdAt },
        ],
      );
    });

    return { id, name, status: "ready", migrated: true };
  }

  async deleteVault(id: string): Promise<void> {
    await this.client.execute(`DELETE FROM vaults WHERE id = ?`, [
      { type: "text", value: id },
    ]);
  }
}
```

## Acceptance Criteria

- [ ] No IndexedDB usage — all operations on shared SQLite DB
- [ ] All queries use typed `select()` with Valibot parsers
- [ ] `reserveVault` is atomic (single transaction with vault metadata)
- [ ] `deleteVault` cascades to child tables
- [ ] `listVaultIdentities` returns vault list for selector UI

## Dependencies

Blocked by: 01
