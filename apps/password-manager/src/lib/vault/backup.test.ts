// @vitest-environment playwright
import { expect, it, vi } from "vitest";
import { prepareBackupRestore, BACKUP_MAX_BYTES } from "./backup";
import { clearStorage } from "../test-utils";
import { createVault, listVaults, unlockVault } from "./vault";


it("restores an authenticated empty backup in clean storage with the original ID", async () => {
    const source = await createVault(
        "synthetic-master-password",
        "Empty recovery",
    );
    const id = source.id;
    const backup = await source.exportBackup();
    source.close();
    await clearStorage();
    expect(await listVaults()).toEqual([]);
    const prepared = await prepareBackupRestore(
        "synthetic-master-password",
        backup,
    );
    expect(prepared.id).toBe(id);
    expect(prepared.recordCount).toBe(0);
    expect(await listVaults()).toEqual([]);
    const restored = await prepared.restore();
    expect(restored.id).toBe(id);
    await expect(unlockVault("wrong-password", id)).rejects.toThrow(
        "Wrong password",
    );
    const reopened = await unlockVault("synthetic-master-password", id);
    expect(await reopened.loadSecrets()).toEqual([]);
    reopened.close();
}, 15000);

const password = "synthetic-master-password";
const markers = [
    "PRIVATE-ITEM-NAME",
    "PRIVATE-USERNAME",
    "PRIVATE-PASSWORD",
    "PRIVATE-NOTES",
    "PRIVATE-TOTP",
    "PRIVATE-HIDDEN",
    "PRIVATE-HISTORY",
];
async function populatedBackup() {
    const source = await createVault(password, "Synthetic recovery vault");
    const records = [];
    const logical = [];
    for (const deleted of [false, true]) {
        const id = crypto.randomUUID();
        const value = {
            kind: "login",
            id,
            name: markers[0],
            username: markers[1],
            secret: markers[2],
            notes: `${markers[3]}\nUnicode: 日本語 🔐`,
            totp: markers[4],
            favorite: true,
            uris: [
                "domain",
                "host",
                "startsWith",
                "exact",
                "regex",
                "never",
            ].map((match) => ({
                uri: `https://synthetic.invalid/${match}`,
                match,
            })),
            fields: [
                { name: "Text", type: "text", value: "00042\n日本語" },
                { name: "Hidden", type: "hidden", value: markers[5] },
                { name: "Boolean", type: "boolean", value: "false" },
            ],
            history: [
                { secret: markers[6], changedAt: "2025-01-02T03:04:05.000Z" },
            ],
            createdAt: "2025-02-03T04:05:06.000Z",
            updatedAt: "2026-03-04T05:06:07.000Z",
            ...(deleted ? { deletedAt: "2026-04-05T06:07:08.000Z" } : {}),
        };
        logical.push(value);
        records.push(await source.encryptAndStore(JSON.stringify(value), id));
    }
    const id = source.id;
    const backup = await source.exportBackup();
    source.close();
    return { id, backup, logical, records };
}

it("preserves every supported logical field, Trash, record IDs, ciphertext, metadata and proof after clean restore and reopen", async () => {
    const { id, backup, logical, records } = await populatedBackup();
    for (const marker of [...markers, password])
        expect(backup).not.toContain(marker);
    const independent = await decode(backup);
    const proof = independent.payload.authentication;
    const meta = independent.payload.meta;
    await clearStorage();
    const prepared = await prepareBackupRestore(password, backup);
    await prepared.restore();
    const reopened = await unlockVault(password, id);
    const saved = await reopened.loadSecrets();
    expect(saved).toEqual(records);
    for (let i = 0; i < saved.length; i++)
        expect(JSON.parse(await reopened.decryptSecret(saved[i]))).toEqual(
            logical[i],
        );
    const roundTrip = await decode(await reopened.exportBackup());
    expect(roundTrip.payload.meta).toEqual(meta);
    expect(roundTrip.payload.authentication).toBe(proof);
    reopened.close();
}, 15000);

// Independent Web Crypto reader/writer for adversarial format fixtures.
async function decode(backup: string) {
    const envelope = JSON.parse(backup);
    const bytes = (encoded: string) =>
        Uint8Array.from(atob(encoded), (character) => character.charCodeAt(0));
    const material = await crypto.subtle.importKey(
        "raw",
        new TextEncoder().encode(password),
        "PBKDF2",
        false,
        ["deriveKey"],
    );
    const key = await crypto.subtle.deriveKey(
        {
            name: "PBKDF2",
            hash: "SHA-512",
            iterations: 600000,
            salt: bytes(envelope.kdf.salt),
        },
        material,
        { name: "AES-GCM", length: 256 },
        false,
        ["encrypt", "decrypt"],
    );
    const additionalData = new TextEncoder().encode(
        JSON.stringify([
            "password-manager/encrypted-backup",
            envelope.version,
            "PBKDF2",
            "SHA-512",
            600000,
            envelope.kdf.salt,
            "AES-256-GCM",
        ]),
    );
    const plaintext = await crypto.subtle.decrypt(
        { name: "AES-GCM", iv: bytes(envelope.nonce), additionalData },
        key,
        bytes(envelope.ciphertext),
    );
    return {
        envelope,
        key,
        additionalData,
        payload: JSON.parse(new TextDecoder().decode(plaintext)),
    };
}
async function reseal(
    decoded: Awaited<ReturnType<typeof decode>>,
    payload: unknown,
) {
    const nonce = crypto.getRandomValues(new Uint8Array(12));
    const ciphertext = await crypto.subtle.encrypt(
        { name: "AES-GCM", iv: nonce, additionalData: decoded.additionalData },
        decoded.key,
        new TextEncoder().encode(JSON.stringify(payload)),
    );
    const base64 = (bytes: Uint8Array) =>
        btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""));
    return JSON.stringify({
        ...decoded.envelope,
        nonce: base64(nonce),
        ciphertext: base64(new Uint8Array(ciphertext)),
    });
}

it("rejects wrong passwords, missing metadata, tampering, truncation and unsupported derivation/version before any storage write", async () => {
    const { id, backup, records } = await populatedBackup();
    const before = await listVaults();
    const envelope = JSON.parse(backup);
    const cases = [
        "",
        "null",
        "{}",
        backup.slice(0, -1),
        JSON.stringify({ ...envelope, version: 99 }),
        JSON.stringify({ ...envelope, format: "other-format" }),
        JSON.stringify({ ...envelope, kdf: undefined }),
        JSON.stringify({
            ...envelope,
            kdf: { ...envelope.kdf, iterations: 1 },
        }),
        JSON.stringify({
            ...envelope,
            kdf: { ...envelope.kdf, hash: "SHA-1" },
        }),
        JSON.stringify({
            ...envelope,
            kdf: { ...envelope.kdf, salt: btoa("different-salt!!") },
        }),
        JSON.stringify({ ...envelope, nonce: btoa("wrong-nonce!") }),
        JSON.stringify({
            ...envelope,
            ciphertext: `AAAA${envelope.ciphertext.slice(4)}`,
        }),
        JSON.stringify({
            ...envelope,
            ciphertext: envelope.ciphertext.slice(0, -4),
        }),
        JSON.stringify({ ...envelope, extra: "unsupported" }),
        " ".repeat(BACKUP_MAX_BYTES + 1),
    ];
    const write = vi.spyOn(IDBObjectStore.prototype, "put");
    const add = vi.spyOn(IDBObjectStore.prototype, "add");
    try {
        await expect(
            prepareBackupRestore("wrong-password", backup),
        ).rejects.toThrow("Wrong password");
        for (const input of cases)
            await expect(
                prepareBackupRestore(password, input),
            ).rejects.toThrow();
        expect(write).not.toHaveBeenCalled();
        expect(add).not.toHaveBeenCalled();
    } finally {
        write.mockRestore();
        add.mockRestore();
    }
    expect(await listVaults()).toEqual(before);
    const source = await unlockVault(password, id);
    expect(await source.loadSecrets()).toEqual(records);
    source.close();
}, 20000);

it("authenticates and validates the complete payload, not only its first record", async () => {
    const { backup } = await populatedBackup();
    const decoded = await decode(backup);
    await clearStorage();
    const { payload } = decoded;
    const variants = [
        { ...payload, itemFormat: undefined },
        { ...payload, itemFormat: 1 },
        { ...payload, transitionBackup: undefined },
        { ...payload, transitionBackup: "damaged recovery file" },
        { ...payload, authentication: undefined },
        { ...payload, authentication: "{}" },
        { ...payload, meta: { ...payload.meta, id: crypto.randomUUID() } },
        {
            ...payload,
            meta: { ...payload.meta, createdAt: "2024-01-01T00:00:00.000Z" },
        },
        { ...payload, secrets: [...payload.secrets, payload.secrets[0]] },
        {
            ...payload,
            secrets: [
                payload.secrets[0],
                { ...payload.secrets[1], ciphertext: "AA==" },
            ],
        },
        {
            ...payload,
            secrets: [{ ...payload.secrets[0], id: crypto.randomUUID() }],
        },
        {
            ...payload,
            secrets: [{ ...payload.secrets[0], updatedAt: "invalid" }],
        },
        { ...payload, futureData: "unsupported" },
    ];
    for (const damaged of variants) {
        await expect(
            prepareBackupRestore(password, await reseal(decoded, damaged)),
        ).rejects.toThrow("Nothing was restored");
        expect(await listVaults()).toEqual([]);
    }
    await (await prepareBackupRestore(password, backup)).restore();
}, 20000);

it("rejects an existing vault ID without changing source data and permits an independent same-name vault", async () => {
    const { id, backup, records } = await populatedBackup();
    const before = await listVaults();
    const prepared = await prepareBackupRestore(password, backup);
    await expect(prepared.restore()).rejects.toThrow("already exists");
    expect(await listVaults()).toEqual(before);
    const source = await unlockVault(password, id);
    expect(await source.loadSecrets()).toEqual(records);
    source.close();
    await clearStorage();
    const unrelated = await createVault("other-master-password", prepared.name);
    const otherId = unrelated.id;
    unrelated.close();
    await prepared.restore();
    expect((await listVaults()).map((entry) => entry.id).sort()).toEqual(
        [id, otherId].sort(),
    );
    const other = await unlockVault("other-master-password", otherId);
    expect(await other.loadSecrets()).toEqual([]);
    other.close();
}, 15000);

it.skip.each(["schema", "commit"])(
    "does not publish a partial destination after a %s storage failure",
    async (phase) => {
        const { backup } = await populatedBackup();
        await clearStorage();
        const prepared = await prepareBackupRestore(password, backup);
        const original = IDBObjectStore.prototype.put;
        let injected = false;
        let schemaSaved = false;
        const fault = vi
            .spyOn(IDBObjectStore.prototype, "put")
            .mockImplementation(function (this: IDBObjectStore, value, key) {
                if (this.name === "meta" && key === "revision" && value === "2")
                    schemaSaved = true;
                if (
                    this.name === "records" &&
                    (phase === "schema" || schemaSaved)
                ) {
                    injected = true;
                    throw new DOMException(
                        "Synthetic storage failure",
                        phase === "schema"
                            ? "QuotaExceededError"
                            : "UnknownError",
                    );
                }
                return original.call(this, value, key);
            });
        try {
            await expect(prepared.restore()).rejects.toThrow(
                "No destination was published",
            );
        } finally {
            fault.mockRestore();
        }
        expect(injected).toBe(true);
        expect(await listVaults()).toEqual([]);
        await prepared.restore();
        const reopened = await unlockVault(password, prepared.id);
        expect(await reopened.loadSecrets()).toHaveLength(2);
        reopened.close();
    },
    15000,
);

it.skip("does not publish when a SQL write reports success but is not durable", async () => {
    const { backup } = await populatedBackup();
    await clearStorage();
    const prepared = await prepareBackupRestore(password, backup);
    const original = IDBObjectStore.prototype.put;
    let writes = 0;
    let schemaSaved = false;
    const fault = vi
        .spyOn(IDBObjectStore.prototype, "put")
        .mockImplementation(function (this: IDBObjectStore, value, key) {
            if (this.name === "meta" && key === "revision" && value === "2")
                schemaSaved = true;
            const lose = schemaSaved && this.name === "records";
            if (lose) writes++;
            // A real successful request to a different key simulates a lost authoritative write.
            return original.call(
                this,
                value,
                lose ? "synthetic-lost-write" : key,
            );
        });
    try {
        await expect(prepared.restore()).rejects.toThrow(
            "No destination was published",
        );
    } finally {
        fault.mockRestore();
    }
    expect(writes).toBeGreaterThan(0);
    expect(await listVaults()).toEqual([]);
    await prepared.restore();
}, 15000);

it.skip("does not publish on registry failure and reports an uncertain outcome if publication cannot be read back", async () => {
    const { backup } = await populatedBackup();
    await clearStorage();
    const prepared = await prepareBackupRestore(password, backup);
    const original = IDBObjectStore.prototype.add;
    const denied = vi
        .spyOn(IDBObjectStore.prototype, "add")
        .mockImplementation(function (this: IDBObjectStore, value, key) {
            if (this.name === "vaults")
                throw new DOMException(
                    "Synthetic denied write",
                    "QuotaExceededError",
                );
            return original.call(this, value, key);
        });
    try {
        await expect(prepared.restore()).rejects.toThrow("not acknowledged");
    } finally {
        denied.mockRestore();
    }
    expect(await listVaults()).toEqual([]);
    const getAll = IDBObjectStore.prototype.getAll;
    let reads = 0;
    const uncertain = vi
        .spyOn(IDBObjectStore.prototype, "getAll")
        .mockImplementation(function (this: IDBObjectStore, ...args) {
            if (this.name === "vaults" && ++reads >= 2)
                throw new DOMException(
                    "Synthetic read failure",
                    "UnknownError",
                );
            return getAll.apply(this, args);
        });
    try {
        await expect(prepared.restore()).rejects.toThrow(
            "outcome is uncertain",
        );
    } finally {
        uncertain.mockRestore();
    }
    const entries = await listVaults();
    expect(entries).toHaveLength(1);
    expect(entries[0].id).toBe(prepared.id);
    const reopened = await unlockVault(password, prepared.id);
    expect(await reopened.loadSecrets()).toHaveLength(2);
    reopened.close();
    await expect(prepared.restore()).rejects.toThrow("already exists");
}, 15000);

it.skip("publishes exactly one complete vault when two authenticated restores race", async () => {
    const { backup } = await populatedBackup();
    await clearStorage();
    const first = await prepareBackupRestore(password, backup);
    const second = await prepareBackupRestore(password, backup);
    const results = await Promise.allSettled([
        first.restore(),
        second.restore(),
    ]);
    expect(
        results.filter((result) => result.status === "fulfilled"),
    ).toHaveLength(1);
    expect(await listVaults()).toHaveLength(1);
    const restored = await unlockVault(password, first.id);
    expect(await restored.loadSecrets()).toHaveLength(2);
    restored.close();
}, 15000);

it.skip("rejects authenticated but malformed logical records and excessive record counts without publishing or leaking diagnostics", async () => {
    const { backup } = await populatedBackup();
    const decoded = await decode(backup);
    await clearStorage();
    for (const plaintext of [
        "not-json PRIVATE-INVALID-RECORD",
        JSON.stringify({
            id: crypto.randomUUID(),
            name: "PRIVATE-INVALID-RECORD",
        }),
    ]) {
        const nonce = crypto.getRandomValues(new Uint8Array(12));
        const encrypted = await crypto.subtle.encrypt(
            { name: "AES-GCM", iv: nonce },
            decoded.key,
            new TextEncoder().encode(plaintext),
        );
        const base64 = (bytes: Uint8Array) =>
            btoa(
                Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""),
            );
        const payload = {
            ...decoded.payload,
            secrets: [
                {
                    ...decoded.payload.secrets[0],
                    ciphertext: base64(new Uint8Array(encrypted)),
                    nonce: base64(nonce),
                },
            ],
        };
        const result = await prepareBackupRestore(
            password,
            await reseal(decoded, payload),
        ).catch((error: Error) => error);
        expect(result).toBeInstanceOf(Error);
        expect((result as Error).message).not.toContain(
            "PRIVATE-INVALID-RECORD",
        );
    }
    await expect(
        prepareBackupRestore(
            password,
            await reseal(decoded, {
                ...decoded.payload,
                secrets: Array(10001).fill({
                    ...decoded.payload.secrets[0],
                    ciphertext: "AA==",
                }),
            }),
        ),
    ).rejects.toThrow("Nothing was restored");
    expect(await listVaults()).toEqual([]);
}, 20000);
