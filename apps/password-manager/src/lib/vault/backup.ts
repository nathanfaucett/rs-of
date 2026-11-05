import { Database, type DbValue } from "@ofsdk/sql";
import * as v from "valibot";
import { transitionLogin } from "../secrets/legacy-login";
import { SecretSchema } from "../secrets/secret";
import {
    decrypt,
    deriveKey,
    encrypt,
    fromBase64,
    PBKDF2_ITERATIONS,
    toBase64,
    verifyAuthentication,
} from "./crypto";
import { vaultMigrations } from "./migrations";
import { decryptLockSettings } from "./lock";
import {
    listVaultIdentities,
    publishRestoredVault,
    type VaultIdentity,
} from "./registry";

export const BACKUP_MAX_BYTES = 16 * 1024 * 1024;
const MAX_RECORDS = 10_000;
const FORMAT = "password-manager/encrypted-backup";
const invalid = () =>
    new Error(
        "Invalid or unsupported encrypted backup. Keep the original file and source vault. Use a complete supported backup within the 16 MiB / 10,000 record limit.",
    );
const text = v.pipe(v.string(), v.minLength(1));
const date = v.pipe(v.string(), v.isoTimestamp());
const uuid = v.pipe(v.string(), v.uuid());
const metaSchema = v.strictObject({
    id: uuid,
    salt: text,
    iv: text,
    createdAt: date,
});
const recordSchema = v.strictObject({
    id: uuid,
    ciphertext: text,
    nonce: text,
    createdAt: date,
    updatedAt: date,
});
const payloadSchema = v.strictObject({
    lockSettings: v.optional(v.nullable(text)),
    itemFormat: v.optional(v.literal(2)),
    transitionBackup: v.optional(v.nullable(text)),
    meta: metaSchema,
    authentication: text,
    name: v.pipe(
        text,
        v.check((name) => !!name.trim()),
    ),
    secrets: v.pipe(v.array(recordSchema), v.maxLength(MAX_RECORDS)),
});
type BackupPayload = v.InferOutput<typeof payloadSchema>;
const envelopeSchema = v.strictObject({
    format: v.literal(FORMAT),
    version: v.picklist([1, 2]),
    kdf: v.strictObject({
        name: v.literal("PBKDF2"),
        hash: v.literal("SHA-512"),
        iterations: v.literal(PBKDF2_ITERATIONS),
        salt: text,
    }),
    cipher: v.literal("AES-256-GCM"),
    nonce: text,
    ciphertext: text,
});

function bytes(value: string, length?: number): ArrayBuffer {
    const decoded = fromBase64(value);
    if (
        toBase64(decoded) !== value ||
        (length !== undefined && decoded.byteLength !== length)
    )
        throw invalid();
    return decoded;
}

function aad(salt: string, version: number): Uint8Array<ArrayBuffer> {
    return new TextEncoder().encode(
        JSON.stringify([
            FORMAT,
            version,
            "PBKDF2",
            "SHA-512",
            PBKDF2_ITERATIONS,
            salt,
            "AES-256-GCM",
        ]),
    );
}

async function validatePayload(
    key: CryptoKey,
    input: unknown,
): Promise<BackupPayload> {
    const payload = v.parse(payloadSchema, input);
    if (payload.itemFormat === 2 && !Object.hasOwn(payload, "transitionBackup"))
        throw invalid();
    bytes(payload.meta.salt, 16);
    bytes(payload.meta.iv, 12);
    await verifyAuthentication(key, payload.meta, payload.authentication);
    await decryptLockSettings(key, payload.meta.id, payload.lockSettings ?? null);
    if (payload.transitionBackup != null) {
        if (payload.itemFormat !== 2) throw invalid();
        await validateTransitionBackup(key, payload, payload.transitionBackup);
    }
    const ids = new Set<string>();
    for (const record of payload.secrets) {
        if (ids.has(record.id)) throw invalid();
        ids.add(record.id);
        bytes(record.nonce, 12);
        bytes(record.ciphertext);
        const plaintext = await decrypt(key, record.ciphertext, record.nonce);
        const logical =
            payload.itemFormat === 2
                ? v.parse(SecretSchema, JSON.parse(plaintext))
                : transitionLogin(JSON.parse(plaintext));
        if (logical.id !== record.id) throw invalid();
    }
    return payload;
}

export async function validateTransitionBackup(
    key: CryptoKey,
    current: Pick<BackupPayload, "meta" | "authentication">,
    backup: string,
): Promise<void> {
    try {
        if (new TextEncoder().encode(backup).byteLength > BACKUP_MAX_BYTES)
            throw invalid();
        const envelope = v.parse(envelopeSchema, JSON.parse(backup));
        if (envelope.version !== 1 || envelope.kdf.salt !== current.meta.salt)
            throw invalid();
        bytes(envelope.nonce, 12);
        bytes(envelope.ciphertext);
        const source = JSON.parse(
            await decrypt(
                key,
                envelope.ciphertext,
                envelope.nonce,
                aad(envelope.kdf.salt, 1),
            ),
        );
        if (
            source.itemFormat !== undefined ||
            source.transitionBackup !== undefined
        )
            throw invalid();
        const recovery = await validatePayload(key, source);
        if (
            JSON.stringify(recovery.meta) !== JSON.stringify(current.meta) ||
            recovery.authentication !== current.authentication
        )
            throw invalid();
    } catch {
        throw new Error(
            "Cannot validate the pre-transition backup. Keep the original source database and use a verified external backup. Do not clear browser data.",
        );
    }
}

export async function encodeBackup(
    key: CryptoKey,
    input: BackupPayload,
): Promise<string> {
    try {
        const payload = await validatePayload(key, input);
        const encrypted = await encrypt(
            key,
            JSON.stringify(payload),
            aad(payload.meta.salt, payload.itemFormat === 2 ? 2 : 1),
        );
        const backup = JSON.stringify({
            format: FORMAT,
            version: payload.itemFormat === 2 ? 2 : 1,
            kdf: {
                name: "PBKDF2",
                hash: "SHA-512",
                iterations: PBKDF2_ITERATIONS,
                salt: payload.meta.salt,
            },
            cipher: "AES-256-GCM",
            ...encrypted,
        });
        if (new TextEncoder().encode(backup).byteLength > BACKUP_MAX_BYTES)
            throw invalid();
        return backup;
    } catch {
        throw new Error(
            "Cannot create a complete encrypted backup. Check the vault records and the 16 MiB / 10,000 record limit. No source data was changed.",
        );
    }
}

export interface BackupRestore {
    readonly id: string;
    readonly name: string;
    readonly recordCount: number;
    restore(): Promise<VaultIdentity>;
}

export async function prepareBackupRestore(
    password: string,
    backup: string,
): Promise<BackupRestore> {
    if (new TextEncoder().encode(backup).byteLength > BACKUP_MAX_BYTES)
        throw invalid();
    let key: CryptoKey;
    let payload: BackupPayload;
    try {
        const envelope = v.parse(envelopeSchema, JSON.parse(backup));
        const salt = bytes(envelope.kdf.salt, 16);
        bytes(envelope.nonce, 12);
        bytes(envelope.ciphertext);
        key = await deriveKey(password, salt);
        const plaintext = await decrypt(
            key,
            envelope.ciphertext,
            envelope.nonce,
            aad(envelope.kdf.salt, envelope.version),
        );
        payload = await validatePayload(key, JSON.parse(plaintext));
        if (
            payload.meta.salt !== envelope.kdf.salt ||
            (envelope.version === 2) !== (payload.itemFormat === 2)
        )
            throw invalid();
        if (envelope.version === 1 && payload.transitionBackup !== undefined)
            throw invalid();
    } catch {
        throw new Error(
            "Wrong password or invalid encrypted backup. Check the password, complete file, supported version, and 16 MiB / 10,000 record limit. Nothing was restored.",
        );
    }
    let busy = false;
    return Object.freeze({
        id: payload.meta.id,
        name: payload.name,
        recordCount: payload.secrets.length,
        async restore() {
            if (busy)
                throw new Error(
                    "Restore is already running. Check the selector before retrying.",
                );
            busy = true;
            try {
                if (
                    (await listVaultIdentities()).some(
                        (entry) => entry.id === payload.meta.id,
                    )
                ) {
                    throw new Error(
                        "This vault ID already exists. Nothing was overwritten. Select the existing vault, or restore in a clean browser profile. Do not delete the source vault.",
                    );
                }
                const identity: VaultIdentity = {
                    id: payload.meta.id,
                    name: payload.name,
                    databaseName: `password-manager-restore-${crypto.randomUUID()}`,
                    status: "ready",
                };
                await stageVault(identity.databaseName, key, payload);
                await publishRestoredVault(identity);
                return identity;
            } finally {
                busy = false;
            }
        },
    });
}

const params = (values: string[]): DbValue[] =>
    values.map((value) => ({ type: "text", value }));

export async function stageVault(
    name: string,
    key: CryptoKey,
    payload: BackupPayload,
): Promise<void> {
    try {
        const db = await Database.open(name);
        const client = db.client();
        try {
            await client.migrate(vaultMigrations);
            const tx = await client.transaction();
            try {
                await tx.execute(
                    "INSERT INTO vault_meta (id, vault_id, salt, iv, created_at, authentication, item_format, transition_backup, lock_settings) VALUES (1, $1, $2, $3, $4, $5, $6, $7, $8)",
                    [
                        ...params([
                            payload.meta.id,
                            payload.meta.salt,
                            payload.meta.iv,
                            payload.meta.createdAt,
                            payload.authentication,
                        ]),
                        {
                            type: "integer",
                            value: String(payload.itemFormat ?? 1),
                        },
                        payload.transitionBackup
                            ? { type: "text", value: payload.transitionBackup }
                            : { type: "null" },
                        payload.lockSettings ? { type: "text", value: payload.lockSettings } : { type: "null" },
                    ],
                );
                for (const record of payload.secrets) {
                    await tx.execute(
                        "INSERT INTO secrets (id, ciphertext, nonce, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)",
                        params([
                            record.id,
                            record.ciphertext,
                            record.nonce,
                            record.createdAt,
                            record.updatedAt,
                        ]),
                    );
                }
                await tx.commit();
            } finally {
                tx.close();
            }
        } finally {
            client.close();
            db.close();
        }
        // Only a verified reopened database can become visible in the registry.
        const reopened = await Database.open(name);
        const reader = reopened.client();
        try {
            const results = await reader.execute(
                "SELECT vault_id, salt, iv, created_at, authentication FROM vault_meta WHERE id = 1; SELECT id, ciphertext, nonce, created_at, updated_at FROM secrets ORDER BY id",
            );
            const strings = (row: DbValue[]) =>
                row.map((cell) => {
                    if (cell.type !== "text" && cell.type !== "uuid")
                        throw invalid();
                    return cell.value;
                });
            const meta = results[0]?.rows.map(strings);
            const records = results[1]?.rows.map(strings);
            const expectedMeta = [
                [
                    payload.meta.id,
                    payload.meta.salt,
                    payload.meta.iv,
                    payload.meta.createdAt,
                    payload.authentication,
                ],
            ];
            const expectedRecords = [...payload.secrets]
                .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
                .map((record) => [
                    record.id,
                    record.ciphertext,
                    record.nonce,
                    record.createdAt,
                    record.updatedAt,
                ]);
            if (
                JSON.stringify(meta) !== JSON.stringify(expectedMeta) ||
                JSON.stringify(records) !== JSON.stringify(expectedRecords)
            )
                throw invalid();
            const format = await reader.execute(
                "SELECT item_format, transition_backup, lock_settings FROM vault_meta WHERE id = 1",
            );
            if (
                JSON.stringify(format[0].rows[0][0]) !==
                    JSON.stringify({
                        type: "integer",
                        value: String(payload.itemFormat ?? 1),
                    }) ||
                JSON.stringify(format[0].rows[0][1]) !==
                    JSON.stringify(
                        payload.transitionBackup
                            ? { type: "text", value: payload.transitionBackup }
                            : { type: "null" },
                    )
            )
                throw invalid();
            const storedSettings = format[0].rows[0][2];
            if (JSON.stringify(storedSettings) !== JSON.stringify(payload.lockSettings ? { type: "text", value: payload.lockSettings } : { type: "null" })) throw invalid();
            await decryptLockSettings(key, payload.meta.id, payload.lockSettings ?? null);
            await verifyAuthentication(
                key,
                payload.meta,
                payload.authentication,
            );
        } finally {
            reader.close();
            reopened.close();
        }
    } catch {
        throw new Error(
            "Restore storage write could not be verified. No destination was published. Check browser storage and free space, then retry with the backup. Do not clear source data.",
        );
    }
}
