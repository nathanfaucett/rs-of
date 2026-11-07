import {
	type DbValue,
	type Client as SqlClient,
	Database as SqlDatabase,
} from "@ofsdk/sql";
import * as v from "valibot";
import { transitionLogin } from "../secrets/legacy-login";
import type { Secret } from "../secrets/secret";
import { SecretSchema } from "../secrets/secret";
import { encodeBackup, validateTransitionBackup } from "./backup";
import {
	createAuthentication,
	decrypt,
	deriveKey,
	encrypt,
	fromBase64,
	toBase64,
	verifyAuthentication,
} from "./crypto";
import {
	type Folder,
	FolderSchema,
	type ItemMutation,
	type ItemSnapshot,
	StaleItemMutationError,
	validateItemSnapshot,
} from "./folder";
import {
	decryptLockSettings,
	encryptLockSettings,
	type LockSettings,
} from "./lock";
import { vaultMigrations } from "./migrations";
import {
	completeRegistration,
	findVault,
	listVaultIdentities,
	publishItemTransition,
	reserveVault,
} from "./registry";
import { SharedDb } from "./SharedDb";

export type { VaultIdentity } from "./registry";

function sanitizeUuid(uuid: string): string {
	return uuid.replace(/-/g, "");
}

function tablePrefix(vaultId: string): string {
	return `vault_${sanitizeUuid(vaultId)}_`;
}

const META_TABLE = "meta";
const SECRETS_TABLE = "secrets";
const FOLDERS_TABLE = "folders";

export interface VaultMeta {
	id: string;
	salt: string;
	iv: string;
	createdAt: string;
}

export interface EncryptedSecret {
	id: string;
	ciphertext: string;
	nonce: string;
	createdAt: string;
	updatedAt: string;
}

function itemFormat(row: DbValue[], index: number): number {
	const value = row[index];
	if (
		value?.type === "integer" &&
		(value.value === "1" || value.value === "2")
	)
		return Number(value.value);
	throw new Error(
		"Unsupported item format. Keep the source vault and backup.",
	);
}

function rowToText(row: DbValue[], index: number): string {
	const value = row[index];
	if (value?.type === "text" || value?.type === "uuid") return value.value;
	throw new Error(`Expected text at column ${index}`);
}

export class VaultHandle {
	private key: CryptoKey | null;
	private prepared = false;
	private vaultId: string;
	private client: SqlClient;
	private meta: VaultMeta;

	private constructor(
		key: CryptoKey,
		client: SqlClient,
		meta: VaultMeta,
		vaultId: string,
	) {
		this.key = key;
		this.client = client;
		this.meta = meta;
		this.vaultId = vaultId;
	}

	static create(
		key: CryptoKey,
		client: SqlClient,
		meta: VaultMeta,
		vaultId: string,
	): VaultHandle {
		return new VaultHandle(key, client, meta, vaultId);
	}

	private activeKey(): CryptoKey {
		if (!this.key) throw new Error("Vault is closed");
		return this.key;
	}

	private prefixed(table: string): string {
		return `${tablePrefix(this.vaultId)}${table}`;
	}

	private async verifyVaultExists(): Promise<void> {
		this.activeKey();
		const identity = await findVault(this.id);
		this.activeKey();
		if (identity.id !== this.id) {
			throw new Error("Vault no longer exists in registry");
		}
	}

	get id(): string {
		return this.meta.id;
	}

	async loadSecrets(): Promise<EncryptedSecret[]> {
		this.activeKey();
		const results = await this.client.execute(
			`SELECT id, ciphertext, nonce, created_at, updated_at FROM ${this.prefixed(SECRETS_TABLE)} ORDER BY created_at ASC`,
		);
		this.activeKey();
		const result = results[0];
		if (!result) return [];
		return result.rows.map((row) => ({
			id: rowToText(row, 0),
			ciphertext: rowToText(row, 1),
			nonce: rowToText(row, 2),
			createdAt: rowToText(row, 3),
			updatedAt: rowToText(row, 4),
		}));
	}

	async loadFolders(): Promise<EncryptedSecret[]> {
		this.activeKey();
		const results = await this.client.execute(
			`SELECT id, ciphertext, nonce, created_at, updated_at FROM ${this.prefixed("folders")} ORDER BY created_at ASC`,
		);
		this.activeKey();
		return results[0].rows.map((row) => ({
			id: rowToText(row, 0),
			ciphertext: rowToText(row, 1),
			nonce: rowToText(row, 2),
			createdAt: rowToText(row, 3),
			updatedAt: rowToText(row, 4),
		}));
	}

	private async itemSnapshot(): Promise<ItemSnapshot> {
		try {
			const items: Secret[] = [];
			const folders: Folder[] = [];
			for (const record of await this.loadSecrets()) {
				const item = v.parse(
					SecretSchema,
					JSON.parse(await this.decryptSecret(record)),
				);
				if (item.id !== record.id) throw new Error("Identity mismatch");
				items.push(item);
			}
			for (const record of await this.loadFolders()) {
				const folder = v.parse(
					FolderSchema,
					JSON.parse(await this.decryptSecret(record)),
				);
				if (folder.id !== record.id)
					throw new Error("Identity mismatch");
				folders.push(folder);
			}
			this.activeKey();
			return validateItemSnapshot({ items, folders });
		} catch {
			this.activeKey();
			throw new Error(
				"Malformed vault snapshot. No partial records or folders were loaded.",
			);
		}
	}

	async readItems(): Promise<ItemSnapshot> {
		await this.prepareItems();
		return navigator.locks.request(
			`password-manager-items-${this.id}`,
			async () => {
				await this.verifyVaultExists();
				return this.itemSnapshot();
			},
		);
	}

	async mutateItems(mutation: ItemMutation): Promise<void> {
		await this.prepareItems();
		await this.changeItems((snapshot) => {
			if (
				mutation.expectedSnapshot &&
				JSON.stringify(snapshot) !==
					JSON.stringify(
						validateItemSnapshot(mutation.expectedSnapshot),
					)
			)
				throw new StaleItemMutationError();
			for (const { id, record } of mutation.expectedItems ?? []) {
				const current =
					snapshot.items.find((item) => item.id === id) ?? null;
				if (
					JSON.stringify(current) !==
						JSON.stringify(
							record === null
								? null
								: v.parse(SecretSchema, record),
						) ||
					(mutation.deleteIds?.includes(id) && !current?.deletedAt)
				)
					throw new StaleItemMutationError();
			}
			const update = <T extends { id: string }>(
				previous: T[],
				upsert: T[] = [],
				removed: string[] = [],
			) => {
				if (
					new Set(upsert.map((value) => value.id)).size !==
						upsert.length ||
					new Set(removed).size !== removed.length ||
					upsert.some((value) => removed.includes(value.id))
				)
					throw new Error("Ambiguous batch identities");
				for (const id of removed)
					v.parse(v.pipe(v.string(), v.uuid()), id);
				const values = new Map(
					previous.map((value) => [value.id, value]),
				);
				for (const id of removed) values.delete(id);
				for (const value of upsert) values.set(value.id, value);
				return [...values.values()];
			};
			return {
				items: update(
					snapshot.items,
					mutation.upsert,
					mutation.deleteIds,
				),
				folders: update(
					snapshot.folders,
					mutation.folders,
					mutation.deleteFolderIds,
				),
			};
		});
	}

	private async changeItems(
		change: (snapshot: ItemSnapshot) => ItemSnapshot,
	): Promise<void> {
		await navigator.locks.request(
			`password-manager-items-${this.id}`,
			async () => {
				await this.verifyVaultExists();
				const previous = await this.itemSnapshot();
				const next = validateItemSnapshot(change(previous));
				const encrypted = {
					secrets: await this.loadSecrets(),
					folders: await this.loadFolders(),
				};
				const expected: typeof encrypted = { secrets: [], folders: [] };
				for (const table of ["secrets", "folders"] as const) {
					const values =
						table === "secrets" ? next.items : next.folders;
					const old =
						table === "secrets" ? previous.items : previous.folders;
					for (const value of values) {
						const record = encrypted[table].find(
							(entry) => entry.id === value.id,
						);
						if (
							record &&
							JSON.stringify(
								old.find((entry) => entry.id === value.id),
							) === JSON.stringify(value)
						)
							expected[table].push(record);
						else {
							const now = new Date().toISOString();
							expected[table].push({
								id: value.id,
								...(await encrypt(
									this.activeKey(),
									JSON.stringify(value),
								)),
								createdAt: record?.createdAt ?? now,
								updatedAt: now,
							});
						}
					}
				}
				this.activeKey();
				const identity = await findVault(this.id);
				const state = await this.client.execute(
					`SELECT authentication, transition_backup, lock_settings FROM ${this.prefixed(META_TABLE)} WHERE id = 1`,
				);
				const payload = {
					meta: this.meta,
					authentication: rowToText(state[0].rows[0], 0),
					name: identity.name,
					itemFormat: 2 as const,
					transitionBackup:
						state[0].rows[0][1].type === "text"
							? String(state[0].rows[0][1].value)
							: null,
					secrets: expected.secrets,
					folders: expected.folders,
					lockSettings:
						state[0].rows[0][2].type === "text"
							? rowToText(state[0].rows[0], 2)
							: null,
				};
				await encodeBackup(this.activeKey(), payload);
				await this.writePayloadToSharedDb(payload);
			this.prepared = true;
			},
		);
	}

private async writePayloadToSharedDb(payload: {
	meta: VaultMeta;
	authentication: string;
	name: string;
	itemFormat: number;
	transitionBackup: string | null;
	secrets: EncryptedSecret[];
	folders: EncryptedSecret[];
	lockSettings: string | null;
}): Promise<void> {
	const tx = await this.client.transaction();
	try {
		await tx.execute(
			`UPDATE ${this.prefixed(META_TABLE)} SET authentication = $1, item_format = $2, transition_backup = $3, lock_settings = $4 WHERE id = 1`,
			[
				{ type: "text", value: payload.authentication },
				{ type: "integer", value: String(payload.itemFormat) },
				payload.transitionBackup
					? { type: "text", value: payload.transitionBackup }
					: { type: "null" },
				payload.lockSettings
					? { type: "text", value: payload.lockSettings }
					: { type: "null" },
			],
		);
		for (const record of payload.secrets) {
			await tx.execute(
				`INSERT INTO ${this.prefixed(SECRETS_TABLE)} (id, ciphertext, nonce, created_at, updated_at) VALUES ($1, $2, $3, $4, $5) ON CONFLICT(id) DO UPDATE SET ciphertext = $2, nonce = $3, updated_at = $5`,
				[
					{ type: "text", value: record.id },
					{ type: "text", value: record.ciphertext },
					{ type: "text", value: record.nonce },
					{ type: "text", value: record.createdAt },
					{ type: "text", value: record.updatedAt },
				],
			);
		}
		for (const record of payload.folders) {
			await tx.execute(
				`INSERT INTO ${this.prefixed("folders")} (id, ciphertext, nonce, created_at, updated_at) VALUES ($1, $2, $3, $4, $5) ON CONFLICT(id) DO UPDATE SET ciphertext = $2, nonce = $3, updated_at = $5`,
				[
					{ type: "text", value: record.id },
					{ type: "text", value: record.ciphertext },
					{ type: "text", value: record.nonce },
					{ type: "text", value: record.createdAt },
					{ type: "text", value: record.updatedAt },
				],
			);
		}
		await tx.commit();
	} finally {
		tx.close();
	}
}

async createFolder(name: string): Promise<Folder> {
		const folder = v.safeParse(FolderSchema, {
			id: crypto.randomUUID(),
			name,
		});
		if (!folder.success) throw new Error("A folder name is required.");
		await this.mutateItems({ folders: [folder.output] });
		this.activeKey();
		return folder.output;
	}

	async renameFolder(id: string, name: string): Promise<void> {
		if (!v.safeParse(FolderSchema, { id, name }).success)
			throw new Error("A valid folder and name are required.");
		await this.prepareItems();
		await this.changeItems((snapshot) => {
			if (!snapshot.folders.some((folder) => folder.id === id))
				throw new Error(
					"Folder no longer exists. Refresh before retrying.",
				);
			return {
				...snapshot,
				folders: snapshot.folders.map((folder) =>
					folder.id === id ? { id, name } : folder,
				),
			};
		});
	}

	async moveItems(ids: string[], folderId: string | null): Promise<void> {
		await this.prepareItems();
		await this.changeItems((snapshot) => {
			if (
				ids.length === 0 ||
				new Set(ids).size !== ids.length ||
				ids.some(
					(id) => !snapshot.items.some((item) => item.id === id),
				) ||
				(folderId !== null &&
					!snapshot.folders.some((folder) => folder.id === folderId))
			)
				throw new Error(
					"Item or folder no longer exists. Refresh before retrying.",
				);
			return {
				...snapshot,
				items: snapshot.items.map((item) =>
					ids.includes(item.id)
						? {
								...item,
								folderId,
								updatedAt: new Date().toISOString(),
							}
						: item,
				),
			};
		});
	}

	async deleteFolder(id: string): Promise<void> {
		await this.prepareItems();
		await this.changeItems((snapshot) => {
			if (!snapshot.folders.some((folder) => folder.id === id))
				throw new Error(
					"Folder no longer exists. Refresh before retrying.",
				);
			return {
				folders: snapshot.folders.filter((folder) => folder.id !== id),
				items: snapshot.items.map((item) =>
					item.folderId === id
						? {
								...item,
								folderId: null,
								updatedAt: new Date().toISOString(),
							}
						: item,
				),
			};
		});
	}

	async decryptSecret(encrypted: EncryptedSecret): Promise<string> {
		const plaintext = await decrypt(
			this.activeKey(),
			encrypted.ciphertext,
			encrypted.nonce,
		);
		this.activeKey();
		return plaintext;
	}

	async encryptAndStore(
		plaintext: string,
		id?: string,
		expected?: Secret | null,
	): Promise<EncryptedSecret> {
		this.activeKey();
		if (expected !== undefined) await this.prepareItems();
		if (this.prepared) {
			const result = v.safeParse(
				SecretSchema,
				(() => {
					try {
						return JSON.parse(plaintext);
					} catch {
						return null;
					}
				})(),
			);
			if (!result.success || result.output.id !== id)
				throw new Error("Invalid typed vault record");
			await this.mutateItems({
				upsert: [result.output],
				...(expected !== undefined
					? {
							expectedItems: [
								{ id: result.output.id, record: expected },
							],
						}
					: {}),
			});
			const record = (await this.loadSecrets()).find(
				(entry) => entry.id === id,
			);
			this.activeKey();
			if (!record)
				throw new Error(
					"Write outcome could not be verified. Reopen before retrying.",
				);
			return record;
		}
		return navigator.locks.request(
			`password-manager-items-${this.id}`,
			async () => {
				await this.verifyVaultExists();
				const state = await this.client.execute(
					`SELECT item_format FROM ${this.prefixed(META_TABLE)} WHERE id = 1`,
				);
				if (itemFormat(state[0].rows[0], 0) !== 2)
					throw new Error(
						"Prepare the item transition before writing. Source data was not changed.",
					);

				const secretId = id ?? crypto.randomUUID();
				const now = new Date().toISOString();
				const { ciphertext, nonce } = await encrypt(
					this.activeKey(),
					plaintext,
				);
				this.activeKey();
				await this.client.execute(
					`INSERT INTO ${this.prefixed(SECRETS_TABLE)} (id, ciphertext, nonce, created_at, updated_at) VALUES ($1, $2, $3, $4, $4) ON CONFLICT(id) DO UPDATE SET ciphertext = $2, nonce = $3, updated_at = $4`,
					[
						{ type: "text", value: secretId },
						{ type: "text", value: ciphertext },
						{ type: "text", value: nonce },
						{ type: "text", value: now },
					],
				);
				this.activeKey();
				return {
					id: secretId,
					ciphertext,
					nonce,
					createdAt: now,
					updatedAt: now,
				};
			},
		);
	}

	async deleteSecret(id: string, expected?: Secret): Promise<void> {
		this.activeKey();
		if (expected !== undefined) await this.prepareItems();
		if (this.prepared)
			return this.mutateItems({
				deleteIds: [id],
				...(expected !== undefined
					? { expectedItems: [{ id, record: expected }] }
					: {}),
			});
		await navigator.locks.request(
			`password-manager-items-${this.id}`,
			async () => {
				await this.verifyVaultExists();
				const state = await this.client.execute(
					`SELECT item_format FROM ${this.prefixed(META_TABLE)} WHERE id = 1`,
				);
				if (itemFormat(state[0].rows[0], 0) !== 2)
					throw new Error(
						"Prepare the item transition before deleting. Source data was not changed.",
					);
				this.activeKey();
				await this.client.execute(
					`DELETE FROM ${this.prefixed(SECRETS_TABLE)} WHERE id = $1`,
					[{ type: "text", value: id }],
				);
				this.activeKey();
			},
		);
	}

	async exportBackup(): Promise<string> {
		return navigator.locks.request(
			`password-manager-items-${this.id}`,
			async () => {
				await this.verifyVaultExists();
				const backup = await this.backupSnapshot();
				this.activeKey();
				return backup;
			},
		);
	}

	private async backupSnapshot(): Promise<string> {
		const result = await this.client.execute(
			`SELECT authentication, item_format, transition_backup, lock_settings FROM ${this.prefixed(META_TABLE)} WHERE id = 1`,
		);
		const authentication = rowToText(result[0].rows[0], 0);
		const identity = await findVault(this.id);
		const format = itemFormat(result[0].rows[0], 1);
		if (format !== 1 && format !== 2)
			throw new Error("Unsupported item format. Keep the source vault.");
		const backup = await encodeBackup(this.activeKey(), {
			lockSettings:
				result[0].rows[0][3].type === "text"
					? rowToText(result[0].rows[0], 3)
					: null,
			...(format === 2
				? {
						itemFormat: 2 as const,
						folders: await this.loadFolders(),
						transitionBackup:
							result[0].rows[0][2].type === "text"
								? (result[0].rows[0][2].value as string)
								: null,
					}
				: {}),
			meta: this.meta,
			authentication,
			name: identity.name,
			secrets: await this.loadSecrets(),
		});
		this.activeKey();
		return backup;
	}

	async exportTransitionBackup(): Promise<string> {
		return navigator.locks.request(
			`password-manager-items-${this.id}`,
			async () => {
				await this.verifyVaultExists();
				const saved = await this.client.execute(
					`SELECT transition_backup, authentication FROM ${this.prefixed(META_TABLE)} WHERE id = 1`,
				);
				const value = saved[0].rows[0][0];
				if (value.type !== "text")
					throw new Error(
						"No pre-transition backup is stored for this vault.",
					);
				await validateTransitionBackup(
					this.activeKey(),
					{
						meta: this.meta,
						authentication: rowToText(saved[0].rows[0], 1),
					},
					value.value,
				);
				this.activeKey();
				return value.value;
			},
		);
	}

	async prepareItems(): Promise<void> {
		if (!navigator.locks)
			throw new Error(
				"This browser cannot protect local writes. Use a browser with Web Locks support. Keep the vault and backup.",
			);
		await navigator.locks.request(
			`password-manager-items-${this.id}`,
			async () => {
				await this.verifyVaultExists();
				const state = await this.client.execute(
					`SELECT item_format FROM ${this.prefixed(META_TABLE)} WHERE id = 1`,
				);
				const format = itemFormat(state[0].rows[0], 0);
				if (format !== 1 && format !== 2)
					throw new Error(
						"Unsupported item format. Keep the source vault and backup.",
					);
				const source = await this.loadSecrets();
				const seen = new Set<string>();
				const target: EncryptedSecret[] = [];
				try {
					for (const record of source) {
						const input = JSON.parse(
							await this.decryptSecret(record),
						);
						const item =
							format === 1
								? transitionLogin(input)
								: v.parse(SecretSchema, input);
						if (item.id !== record.id || seen.has(item.id))
							throw new Error("Invalid identity");
						seen.add(item.id);
						target.push(
							format === 1
								? {
										...record,
										...(await encrypt(
											this.activeKey(),
											JSON.stringify(item),
										)),
									}
								: record,
						);
					}
				} catch {
					throw new Error(
						"Malformed vault record. No partial records were loaded or transitioned. Keep the source vault and recover from a complete backup.",
					);
				}
				this.activeKey();
				if (format === 2) {
					await this.itemSnapshot();
					this.activeKey();
					this.prepared = true;
					return;
				}
				const backup = await this.backupSnapshot();
				const identity = await findVault(this.id);
				const authentication = await this.client.execute(
					`SELECT authentication, lock_settings FROM ${this.prefixed(META_TABLE)} WHERE id = 1`,
				);
				const payload = {
					meta: this.meta,
					authentication: rowToText(authentication[0].rows[0], 0),
					name: identity.name,
					itemFormat: 2 as const,
					transitionBackup: backup,
					secrets: target,
					folders: await this.loadFolders(),
					lockSettings:
						authentication[0].rows[0][1].type === "text"
							? rowToText(authentication[0].rows[0], 1)
							: null,
				};
				await encodeBackup(this.activeKey(), payload);
				await this.writePayloadToSharedDb(payload);
			this.prepared = true;
			this.prepared = true;
			},
		);
	}

	async loadLockSettings(): Promise<LockSettings> {
		return navigator.locks.request(
			`password-manager-items-${this.id}`,
			async () => {
				await this.verifyVaultExists();
				const result = await this.client.execute(
					`SELECT lock_settings FROM ${this.prefixed(META_TABLE)} WHERE id = 1`,
				);
				const cell = result[0].rows[0][0];
				const settings = await decryptLockSettings(
					this.activeKey(),
					this.id,
					cell.type === "null"
						? null
						: rowToText(result[0].rows[0], 0),
				);
				this.activeKey();
				return settings;
			},
		);
	}

	async saveLockSettings(settings: LockSettings): Promise<LockSettings> {
		return navigator.locks.request(
			`password-manager-items-${this.id}`,
			async () => {
				await this.verifyVaultExists();
				const saved = await encryptLockSettings(
					this.activeKey(),
					this.id,
					settings,
				);
				this.activeKey();
				try {
					await this.client.execute(
						`UPDATE ${this.prefixed(META_TABLE)} SET lock_settings = $1 WHERE id = 1`,
						[{ type: "text", value: saved }],
					);
					this.activeKey();
					const result = await this.client.execute(
						`SELECT lock_settings FROM ${this.prefixed(META_TABLE)} WHERE id = 1`,
					);
					this.activeKey();
					if (rowToText(result[0].rows[0], 0) !== saved)
						throw new Error("Unacknowledged settings");
					const confirmed = await decryptLockSettings(
						this.activeKey(),
						this.id,
						saved,
					);
					this.activeKey();
					return confirmed;
				} catch {
					this.activeKey();
					throw new Error(
						"Lock settings write could not be confirmed; it may have persisted. Lock, reopen, and verify before retrying.",
					);
				}
			},
		);
	}

	close(): void {
		if (!this.key) return;
		this.key = null;
		this.prepared = false;

	}
}

export async function createVault(
	password: string,
	name: string,
): Promise<VaultHandle> {
	const identity = await reserveVault(name);
	const salt = crypto.getRandomValues(new Uint8Array(16));
	const iv = crypto.getRandomValues(new Uint8Array(12));
	const vaultId = identity.id;
	const key = await deriveKey(password, salt.buffer);

	const sharedDb = SharedDb.getInstance();
	const client = await sharedDb.getClient(vaultId);

	const meta: VaultMeta = {
		id: vaultId,
		salt: toBase64(salt.buffer),
		iv: toBase64(iv.buffer),
		createdAt: new Date().toISOString(),
	};
	const authentication = await createAuthentication(key, meta);
	try {
		await client.execute(
			`INSERT INTO ${tablePrefix(vaultId)}meta (id, vault_id, salt, iv, created_at, authentication, item_format) VALUES (1, $1, $2, $3, $4, $5, 2)`,
			[
				{ type: "text", value: vaultId },
				{ type: "text", value: meta.salt },
				{ type: "text", value: meta.iv },
				{ type: "text", value: meta.createdAt },
				{ type: "text", value: authentication },
			],
		);
	} catch {
		throw new Error(
			"Could not confirm vault creation. Check local storage access and reopen the vault before retrying. Do not delete existing vault data.",
		);
	}
	await completeRegistration(identity);
	return VaultHandle.create(key, client, meta, vaultId);
}

export async function unlockVault(
	password: string,
	id: string,
): Promise<VaultHandle> {
	const identity = await findVault(id);

	const sharedDb = SharedDb.getInstance();

	const oldDbName = `password-manager-local-${id}`;
	let hasOldDb = false;
	try {
		// Try to open the legacy database directly - more reliable than indexedDB.databases()
		const oldDb = await SqlDatabase.open(oldDbName);
		oldDb.close();
		hasOldDb = true;
	} catch {
		// Legacy database doesn't exist
	}

	if (hasOldDb && !identity.migrated) {
		await sharedDb.migrateVault(oldDbName, id);
		await listVaultIdentities(); // Refresh registry to update migrated flag
	}

	const client = await sharedDb.getClient(id);

	const results = await client.execute(
		`SELECT vault_id, salt, iv, created_at, authentication FROM ${tablePrefix(id)}meta WHERE id = 1`,
	);
	const result = results[0];
	if (!result || result.rows.length === 0) {
		throw new Error(
			"Vault creation was interrupted before metadata was saved. Create a new vault with a new identity. Do not clear browser data.",
		);
	}
	const row = result.rows[0];
	const meta: VaultMeta = {
		id: rowToText(row, 0),
		salt: rowToText(row, 1),
		iv: rowToText(row, 2),
		createdAt: rowToText(row, 3),
	};
	if (meta.id !== identity.id) {
		throw new Error(
			"Vault identity does not match local storage. Reload the selector and check your backup; this vault was not changed.",
		);
	}
	let key: CryptoKey;
	try {
		const salt = fromBase64(meta.salt);
		if (salt.byteLength !== 16)
			throw new Error("Invalid derivation salt");
		key = await deriveKey(password, salt);
	} catch {
		throw new Error(
			"Damaged vault derivation metadata. Keep the vault and recover from a backup.",
		);
	}

	if (row[4]?.type === "null") {
		const secrets = await client.execute(
			`SELECT nonce, ciphertext FROM ${tablePrefix(id)}secrets`,
		);
		if (!secrets[0]?.rows.length) {
			throw new Error(
				"This legacy empty vault has no password proof. Its original password cannot be verified. Keep it unchanged and create a separate new vault. No password was set.",
			);
		}
		try {
			for (const secretRow of secrets[0].rows) {
				await decrypt(
					key,
					rowToText(secretRow, 1),
					rowToText(secretRow, 0),
				);
			}
		} catch {
			throw new Error(
				"Wrong password or damaged legacy records. Retry with the correct password. Keep the vault if this continues.",
			);
		}
		const authentication = await createAuthentication(key, meta);
		try {
			await client.execute(
				`UPDATE ${tablePrefix(id)}meta SET authentication = $1 WHERE id = 1 AND authentication IS NULL`,
				[{ type: "text", value: authentication }],
			);
		} catch {
			throw new Error(
				"Could not confirm the vault authentication upgrade. Check local storage access and reopen the vault before retrying. Existing encrypted records were not changed.",
			);
		}
		const saved = await client.execute(
			`SELECT authentication FROM ${tablePrefix(id)}meta WHERE id = 1`,
		);
		await verifyAuthentication(
			key,
			meta,
			rowToText(saved[0].rows[0], 0),
		);
	} else {
		if (row[4]?.type !== "text") {
			throw new Error(
				"Damaged vault authentication. Keep the vault and recover from a backup.",
			);
		}
		await verifyAuthentication(key, meta, row[4].value);
	}

	await completeRegistration(identity);
	return VaultHandle.create(key, client, meta, id);
}

export { listVaultIdentities as listVaults } from "./registry";
