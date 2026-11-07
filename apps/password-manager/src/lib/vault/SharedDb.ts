import {
	type DbValue,
	type Client as SqlClient,
	Database as SqlDatabase,
} from "@ofsdk/sql";

const SHARED_DB_NAME = "password-manager-v1";

function sanitizeUuid(uuid: string): string {
	return uuid.replace(/-/g, "");
}

function tablePrefix(vaultId: string): string {
	return `vault_${sanitizeUuid(vaultId)}_`;
}

function getTextValue(cell: DbValue): string {
	if (cell.type === "text" || cell.type === "uuid") return cell.value;
	if (cell.type === "integer") return cell.value;
	if (cell.type === "null") return "";
	throw new Error(`Expected text/integer value, got ${cell.type}`);
}

export class SharedDb {
	private static instance: SharedDb | null = null;
	private db: SqlDatabase | null = null;
	private clients = new Map<string, SqlClient>();
	private initializedVaults = new Set<string>();

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

	async getClient(vaultId: string): Promise<SqlClient> {
		const existing = this.clients.get(vaultId);
		if (existing) return existing;

		await this.ensureOpen();
		const client = this.db!.client();
		this.clients.set(vaultId, client);

		await this.ensureTables(vaultId, client);
		return client;
	}

	private async ensureOpen(): Promise<void> {
		if (this.db) return;
		this.db = await SqlDatabase.open(SHARED_DB_NAME);
	}

	private async ensureTables(vaultId: string, client: SqlClient): Promise<void> {
		if (this.initializedVaults.has(vaultId)) return;
		const prefix = tablePrefix(vaultId);

		// Create tables with final schema (v5) directly - avoids migration version conflicts
		await client.execute(`
			CREATE TABLE IF NOT EXISTS ${prefix}meta (
				id INTEGER PRIMARY KEY CHECK (id = 1),
				vault_id TEXT NOT NULL,
				salt TEXT NOT NULL,
				iv TEXT NOT NULL,
				created_at TEXT NOT NULL,
				authentication TEXT,
				item_format INTEGER NOT NULL DEFAULT 2,
				transition_backup TEXT,
				lock_settings TEXT
			)
		`);
		await client.execute(`
			CREATE TABLE IF NOT EXISTS ${prefix}secrets (
				id TEXT PRIMARY KEY,
				ciphertext TEXT NOT NULL,
				nonce TEXT NOT NULL,
				created_at TEXT NOT NULL,
				updated_at TEXT NOT NULL
			)
		`);
		await client.execute(`
			CREATE TABLE IF NOT EXISTS ${prefix}folders (
				id TEXT PRIMARY KEY,
				ciphertext TEXT NOT NULL,
				nonce TEXT NOT NULL,
				created_at TEXT NOT NULL,
				updated_at TEXT NOT NULL
			)
		`);

		this.initializedVaults.add(vaultId);
	}

	async migrateVault(oldDbName: string, vaultId: string): Promise<void> {
		await this.ensureOpen();
		const oldDb = await SqlDatabase.open(oldDbName);
		const oldClient = oldDb.client();

		try {
			const metaResult = await oldClient.execute(
				"SELECT vault_id, salt, iv, created_at, authentication, item_format, transition_backup, lock_settings FROM vault_meta WHERE id = 1",
			);
			const secretsResult = await oldClient.execute(
				"SELECT id, ciphertext, nonce, created_at, updated_at FROM secrets",
			);
			const foldersResult = await oldClient.execute(
				"SELECT id, ciphertext, nonce, created_at, updated_at FROM folders",
			);

			oldClient.close();
			oldDb.close();

			const metaRows = metaResult[0]?.rows ?? [];
			const secretsRows = secretsResult[0]?.rows ?? [];
			const foldersRows = foldersResult[0]?.rows ?? [];

			if (metaRows.length === 0) {
				throw new Error("No vault metadata found in old database");
			}

			const meta = metaRows[0] as DbValue[];
			const client = await this.getClient(vaultId);
			const prefix = tablePrefix(vaultId);

			// Check if meta already exists (e.g., from previous migration attempt)
			const existingMeta = await client.execute(
				`SELECT id FROM ${prefix}meta WHERE id = 1`,
			);
			if (existingMeta[0]?.rows.length === 0) {
				await client.execute(
					`INSERT INTO ${prefix}meta (id, vault_id, salt, iv, created_at, authentication, item_format, transition_backup, lock_settings) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?)`,
					[
						meta[0],
						meta[1],
						meta[2],
						meta[3],
						meta[4] ?? { type: "null" },
						meta[5] ?? { type: "integer", value: "1" },
						meta[6] ?? { type: "null" },
						meta[7] ?? { type: "null" },
					],
				);
			}

			// Insert secrets, skipping any that already exist
			for (const row of secretsRows) {
				const existing = await client.execute(
					`SELECT id FROM ${prefix}secrets WHERE id = ?`,
					[row[0]],
				);
				if (existing[0]?.rows.length === 0) {
					await client.execute(
						`INSERT INTO ${prefix}secrets (id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?)`,
						[row[0], row[1], row[2], row[3], row[4]],
					);
				}
			}

			// Insert folders, skipping any that already exist
			for (const row of foldersRows) {
				const existing = await client.execute(
					`SELECT id FROM ${prefix}folders WHERE id = ?`,
					[row[0]],
				);
				if (existing[0]?.rows.length === 0) {
					await client.execute(
						`INSERT INTO ${prefix}folders (id, ciphertext, nonce, created_at, updated_at) VALUES (?, ?, ?, ?, ?)`,
						[row[0], row[1], row[2], row[3], row[4]],
					);
				}
			}

			const verifyMeta = await client.execute(
				`SELECT COUNT(*) FROM ${prefix}meta WHERE id = 1`,
			);
			const verifySecrets = await client.execute(
				`SELECT COUNT(*) FROM ${prefix}secrets`,
			);
			const verifyFolders = await client.execute(
				`SELECT COUNT(*) FROM ${prefix}folders`,
			);

			const metaCount = getTextValue(verifyMeta[0]?.rows[0]?.[0] ?? { type: "null" });
			const secretsCount = getTextValue(verifySecrets[0]?.rows[0]?.[0] ?? { type: "null" });
			const foldersCount = getTextValue(verifyFolders[0]?.rows[0]?.[0] ?? { type: "null" });

			if (
				Number(metaCount) !== 1 ||
				Number(secretsCount) !== secretsRows.length ||
				Number(foldersCount) !== foldersRows.length
			) {
				throw new Error("Migration verification failed: row count mismatch");
			}

			await indexedDB.deleteDatabase(oldDbName);
		} catch (error) {
			oldClient.close();
			oldDb.close();
			throw error;
		}
	}

	async deleteVaultTables(vaultId: string): Promise<void> {
		await this.ensureOpen();
		const client = this.db!.client();
		const prefix = tablePrefix(vaultId);

		await client.execute(`DROP TABLE IF EXISTS ${prefix}secrets`);
		await client.execute(`DROP TABLE IF EXISTS ${prefix}folders`);
		await client.execute(`DROP TABLE IF EXISTS ${prefix}meta`);
		client.close();

		// Remove from cache
		const cachedClient = this.clients.get(vaultId);
		if (cachedClient) {
			this.clients.delete(vaultId);
		}
		this.initializedVaults.delete(vaultId);
	}

	close(): void {
		for (const client of this.clients.values()) {
			client.close();
		}
		this.clients.clear();
		this.initializedVaults.clear();
		if (this.db) {
			this.db.close();
			this.db = null;
		}
	}
}
