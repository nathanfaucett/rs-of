import {
    type DbValue,
    type Client as SqlClient,
    Database as SqlDatabase,
} from "@ofsdk/sql";
import { vaultMigrations } from "./migrations";
import { completeRegistration, findVault, reserveVault } from "./registry";

export type { VaultIdentity } from "./registry";

const META_TABLE = "vault_meta";
const SECRETS_TABLE = "secrets";
const PBKDF2_ITERATIONS = 600_000;
const AES_KEY_LENGTH = 256;

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

function toBase64(buffer: ArrayBuffer): string {
    const bytes = new Uint8Array(buffer);
    let binary = "";
    for (let i = 0; i < bytes.byteLength; i++) {
        binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
}

function fromBase64(base64: string): ArrayBuffer {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
    }
    return bytes.buffer;
}

async function deriveKey(
    password: string,
    salt: ArrayBuffer,
): Promise<CryptoKey> {
    const encoder = new TextEncoder();
    const keyMaterial = await crypto.subtle.importKey(
        "raw",
        encoder.encode(password),
        "PBKDF2",
        false,
        ["deriveKey"],
    );
    return crypto.subtle.deriveKey(
        {
            name: "PBKDF2",
            salt,
            iterations: PBKDF2_ITERATIONS,
            hash: "SHA-512",
        },
        keyMaterial,
        { name: "AES-GCM", length: AES_KEY_LENGTH },
        false,
        ["encrypt", "decrypt"],
    );
}

async function encrypt(
    key: CryptoKey,
    plaintext: string,
): Promise<{ ciphertext: string; nonce: string }> {
    const encoder = new TextEncoder();
    const nonce = crypto.getRandomValues(new Uint8Array(12));
    const encrypted = await crypto.subtle.encrypt(
        { name: "AES-GCM", iv: nonce },
        key,
        encoder.encode(plaintext),
    );
    return {
        ciphertext: toBase64(encrypted),
        nonce: toBase64(nonce.buffer),
    };
}

async function decrypt(
    key: CryptoKey,
    ciphertext: string,
    nonce: string,
): Promise<string> {
    const decrypted = await crypto.subtle.decrypt(
        { name: "AES-GCM", iv: new Uint8Array(fromBase64(nonce)) },
        key,
        fromBase64(ciphertext),
    );
    return new TextDecoder().decode(decrypted);
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

    async exportEncrypted(): Promise<string> {
        const secrets = await this.loadSecrets();
        return JSON.stringify({
            version: 1,
            vaultId: this.meta.id,
            exportedAt: new Date().toISOString(),
            secrets,
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
        await client.execute(
            `INSERT INTO ${META_TABLE} (id, vault_id, salt, iv, created_at) VALUES (1, $1, $2, $3, $4)`,
            [
                { type: "text", value: vaultId },
                { type: "text", value: meta.salt },
                { type: "text", value: meta.iv },
                { type: "text", value: meta.createdAt },
            ],
        );
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
            `SELECT vault_id, salt, iv, created_at FROM ${META_TABLE} WHERE id = 1`,
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
        const key = await deriveKey(password, fromBase64(meta.salt));

        // Verify the key by attempting to decrypt stored data
        const secrets = await client.execute(
            `SELECT nonce, ciphertext FROM ${SECRETS_TABLE} LIMIT 1`,
        );
        if (secrets[0]?.rows.length > 0) {
            const secretRow = secrets[0].rows[0];
            try {
                await decrypt(
                    key,
                    rowToText(secretRow, 1),
                    rowToText(secretRow, 0),
                );
            } catch {
                throw new Error("Wrong password.");
            }
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
