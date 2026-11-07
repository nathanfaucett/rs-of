import { Database as SqlDatabase, type Client as SqlClient } from "@ofsdk/sql";
import { MIGRATIONS } from "./schema";

const SHARED_DB_NAME = "password-manager-v1";

export class SharedDb {
  private static instance: SharedDb | null = null;
  private db: SqlDatabase | null = null;
  private client: SqlClient | null = null;

  private constructor() {}

  static getInstance(): SharedDb {
    if (!SharedDb.instance) {
      SharedDb.instance = new SharedDb();
    }
    return SharedDb.instance;
  }

  /** Reset singleton for testing */
  static resetForTesting(): void {
    if (SharedDb.instance) {
      SharedDb.instance.close();
      SharedDb.instance = null;
    }
  }

  /**
   * Get the single shared client for all vault operations.
   * Callers add WHERE vault_id = ? to their queries.
   */
  async getClient(): Promise<SqlClient> {
    if (this.client) return this.client;

    await this.ensureOpen();
    this.client = this.db!.client();

    // Run migrations on first open
    await this.client.migrate(MIGRATIONS);

    return this.client;
  }

  /** Alias for clarity - same client, caller adds vault_id filter */
  async getClientForVault(_vaultId: string): Promise<SqlClient> {
    return this.getClient();
  }

  private async ensureOpen(): Promise<void> {
    if (this.db) return;
    this.db = await SqlDatabase.open(SHARED_DB_NAME);
  }

  close(): void {
    if (this.client) {
      this.client.close();
      this.client = null;
    }
    if (this.db) {
      this.db.close();
      this.db = null;
    }
  }
}
