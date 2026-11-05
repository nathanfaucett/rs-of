import {
    type DbValue,
    type Client as SqlClient,
    Database as SqlDatabase,
} from "@ofsdk/sql";
import {
    createAuthentication,
    decrypt,
    deriveKey,
    encrypt,
    fromBase64,
    toBase64,
    verifyAuthentication,
} from "./crypto";
import { encodeBackup } from "./backup";
import { vaultMigrations } from "./migrations";
import { completeRegistration, findVault, reserveVault } from "./registry";

export type { VaultIdentity } from "./registry";

const META_TABLE = "vault_meta";
const SECRETS_TABLE = "secrets";

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

function rowToText(row: DbValue[], index: number): string {
    const value = row[index];
    if (value?.type === "text" || value?.type === "uuid") return value.value;
    throw new Error(`Expected text at column ${index}`);
}

export class VaultHandle {
    private key: CryptoKey;
    private db: SqlDatabase;
    private client: SqlClient;
    private meta: VaultMeta;

    private constructor(
        key: CryptoKey,
        db: SqlDatabase,
        client: SqlClient,
        meta: VaultMeta,
    ) {
        this.key = key;
        this.db = db;
        this.client = client;
        this.meta = meta;
    }

    static create(
        key: CryptoKey,
        db: SqlDatabase,
        client: SqlClient,
        meta: VaultMeta,
    ): VaultHandle {
        return new VaultHandle(key, db, client, meta);
    }

    get id(): string {
        return this.meta.id;
    }

    async loadSecrets(): Promise<EncryptedSecret[]> {
        const results = await this.client.execute(
            `SELECT id, ciphertext, nonce, created_at, updated_at FROM ${SECRETS_TABLE} ORDER BY created_at ASC`,
        );
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

    async decryptSecret(encrypted: EncryptedSecret): Promise<string> {
        return decrypt(this.key, encrypted.ciphertext, encrypted.nonce);
    }

    async encryptAndStore(
        plaintext: string,
        id?: string,
    ): Promise<EncryptedSecret> {
        const secretId = id ?? crypto.randomUUID();
        const now = new Date().toISOString();
        const { ciphertext, nonce } = await encrypt(this.key, plaintext);
        await this.client.execute(
            `INSERT INTO ${SECRETS_TABLE} (id, ciphertext, nonce, created_at, updated_at) VALUES ($1, $2, $3, $4, $4) ON CONFLICT(id) DO UPDATE SET ciphertext = $2, nonce = $3, updated_at = $4`,
            [
                { type: "text", value: secretId },
                { type: "text", value: ciphertext },
                { type: "text", value: nonce },
                { type: "text", value: now },
            ],
        );
        return {
            id: secretId,
            ciphertext,
            nonce,
            createdAt: now,
            updatedAt: now,
        };
    }

    async deleteSecret(id: string): Promise<void> {
        await this.client.execute(
            `DELETE FROM ${SECRETS_TABLE} WHERE id = $1`,
            [{ type: "text", value: id }],
        );
    }

    async exportBackup(): Promise<string> {
        const result = await this.client.execute(
            "SELECT authentication FROM vault_meta WHERE id = 1",
        );
        const authentication = rowToText(result[0].rows[0], 0);
        const identity = await findVault(this.id);
        return encodeBackup(this.key, {
            meta: this.meta,
            authentication,
            name: identity.name,
            secrets: await this.loadSecrets(),
        });
    }

    close(): void {
        this.client.close();
        this.db.close();
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

    const db = await SqlDatabase.open(identity.databaseName);
    const client = db.client();

    try {
        await client.migrate(vaultMigrations);
        const meta: VaultMeta = {
            id: vaultId,
            salt: toBase64(salt.buffer),
            iv: toBase64(iv.buffer),
            createdAt: new Date().toISOString(),
        };
        const authentication = await createAuthentication(key, meta);
        try {
            await client.execute(
                `INSERT INTO ${META_TABLE} (id, vault_id, salt, iv, created_at, authentication) VALUES (1, $1, $2, $3, $4, $5)`,
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
        return VaultHandle.create(key, db, client, meta);
    } catch (error) {
        client.close();
        db.close();
        throw error;
    }
}

export async function unlockVault(
    password: string,
    id: string,
): Promise<VaultHandle> {
    const identity = await findVault(id);
    const db = await SqlDatabase.open(identity.databaseName);
    const client = db.client();

    try {
        await client.migrate(vaultMigrations);
        const results = await client.execute(
            `SELECT vault_id, salt, iv, created_at, authentication FROM ${META_TABLE} WHERE id = 1`,
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
                `SELECT nonce, ciphertext FROM ${SECRETS_TABLE}`,
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
                    `UPDATE ${META_TABLE} SET authentication = $1 WHERE id = 1 AND authentication IS NULL`,
                    [{ type: "text", value: authentication }],
                );
            } catch {
                throw new Error(
                    "Could not confirm the vault authentication upgrade. Check local storage access and reopen the vault before retrying. Existing encrypted records were not changed.",
                );
            }
            const saved = await client.execute(
                `SELECT authentication FROM ${META_TABLE} WHERE id = 1`,
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
        return VaultHandle.create(key, db, client, meta);
    } catch (error) {
        client.close();
        db.close();
        throw error;
    }
}

export { listVaultIdentities as listVaults } from "./registry";
