import { Database as SqlDatabase, type Client as SqlClient, type DbValue } from "@ofdb/sql";

const VAULT_DB_NAME = "password-manager-vault";
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

function escapeSql(value: string): string {
    return value.replace(/'/g, "''");
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

    async encryptAndStore(plaintext: string, id?: string): Promise<EncryptedSecret> {
        const secretId = id ?? crypto.randomUUID();
        const now = new Date().toISOString();
        const { ciphertext, nonce } = await encrypt(this.key, plaintext);
        const tx = await this.client.transaction();
        try {
            await tx.execute(
                `INSERT INTO ${SECRETS_TABLE} VALUES ('${escapeSql(secretId)}', '${escapeSql(ciphertext)}', '${escapeSql(nonce)}', '${escapeSql(now)}', '${escapeSql(now)}') ON CONFLICT(id) DO UPDATE SET ciphertext = '${escapeSql(ciphertext)}', nonce = '${escapeSql(nonce)}', updated_at = '${escapeSql(now)}'`,
            );
            await tx.commit();
        } catch (error) {
            await tx.rollback().catch(() => {});
            throw error;
        }
        return { id: secretId, ciphertext, nonce, createdAt: now, updatedAt: now };
    }

    async deleteSecret(id: string): Promise<void> {
        const tx = await this.client.transaction();
        try {
            await tx.execute(`DELETE FROM ${SECRETS_TABLE} WHERE id = '${escapeSql(id)}'`);
            await tx.commit();
        } catch (error) {
            await tx.rollback().catch(() => {});
            throw error;
        }
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

export async function createVault(password: string): Promise<VaultHandle> {
        const salt = crypto.getRandomValues(new Uint8Array(16));
        const iv = crypto.getRandomValues(new Uint8Array(12));
        const vaultId = crypto.randomUUID();
        const key = await deriveKey(password, salt.buffer);

        const db = await SqlDatabase.open(VAULT_DB_NAME);
        const client = db.client();

        try {
            const tx = await client.transaction();
            await tx.execute(
                `CREATE TABLE IF NOT EXISTS ${META_TABLE} (id TEXT PRIMARY KEY, salt TEXT NOT NULL, iv TEXT NOT NULL, created_at TEXT NOT NULL)`,
            );
            await tx.execute(
                `CREATE TABLE IF NOT EXISTS ${SECRETS_TABLE} (id TEXT PRIMARY KEY, ciphertext TEXT NOT NULL, nonce TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)`,
            );
            const meta: VaultMeta = {
                id: vaultId,
                salt: toBase64(salt.buffer),
                iv: toBase64(iv.buffer),
                createdAt: new Date().toISOString(),
            };
            await tx.execute(
                `INSERT INTO ${META_TABLE} VALUES ('${escapeSql(vaultId)}', '${escapeSql(meta.salt)}', '${escapeSql(meta.iv)}', '${escapeSql(meta.createdAt)}')`,
            );
            await tx.commit();
            return VaultHandle.create(key, db, client, meta);
        } catch (error) {
            client.close();
            db.close();
            throw error;
        }
}

export async function unlockVault(password: string): Promise<VaultHandle> {
        const db = await SqlDatabase.open(VAULT_DB_NAME);
        const client = db.client();

        try {
            const results = await client.execute(
                `SELECT id, salt, iv, created_at FROM ${META_TABLE} LIMIT 1`,
            );
            const result = results[0];
            if (!result || result.rows.length === 0) {
                throw new Error("No vault found. Create a new vault first.");
            }
            const row = result.rows[0];
            const meta: VaultMeta = {
                id: rowToText(row, 0),
                salt: rowToText(row, 1),
                iv: rowToText(row, 2),
                createdAt: rowToText(row, 3),
            };
            const key = await deriveKey(password, fromBase64(meta.salt));

            // Verify the key by attempting to decrypt stored data
            const secrets = await client.execute(`SELECT nonce, ciphertext FROM ${SECRETS_TABLE} LIMIT 1`);
            if (secrets[0]?.rows.length > 0) {
                const secretRow = secrets[0].rows[0];
                try {
                    await decrypt(key, rowToText(secretRow, 1), rowToText(secretRow, 0));
                } catch {
                    throw new Error("Wrong password.");
                }
            }

            return VaultHandle.create(key, db, client, meta);
        } catch (error) {
            client.close();
            db.close();
            throw error;
        }
}

export async function vaultExists(): Promise<boolean> {
        try {
            const db = await SqlDatabase.open(VAULT_DB_NAME);
            const client = db.client();
            const results = await client.execute(
                `SELECT name FROM sqlite_master WHERE type='table' AND name='${META_TABLE}'`,
            );
            const hasTable = (results[0]?.rows.length ?? 0) > 0;
            client.close();
            db.close();
            return hasTable;
        } catch {
            return false;
        }
}
