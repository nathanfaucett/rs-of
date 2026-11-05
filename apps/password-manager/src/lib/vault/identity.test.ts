import { Database } from "@ofsdk/sql";
import { afterEach, expect, it, vi } from "vitest";
import { vaultMigrations } from "./migrations";

afterEach(() => vi.restoreAllMocks());

import { createVault, listVaults, unlockVault } from "./vault";

it("keeps colliding display names and duplicate names isolated across reopen", async () => {
    const suffix = crypto.randomUUID();
    const names = [
        `Personal ${suffix}`,
        `personal ${suffix}`,
        `Personal!${suffix}`,
        ` Personal ${suffix} `,
        `Personal ${suffix}`,
    ];
    const ids: string[] = [];
    for (const [index, name] of names.entries()) {
        const vault = await createVault(`password-${index}`, name);
        ids.push(vault.id);
        await vault.encryptAndStore(`isolated record ${index}`);
        vault.close();
    }
    expect(new Set(ids).size).toBe(names.length);
    const entries = await listVaults();
    for (const [index, id] of ids.entries()) {
        expect(entries).toContainEqual(
            expect.objectContaining({ id, name: names[index].trim() }),
        );
        const vault = await unlockVault(`password-${index}`, id);
        try {
            const records = await vault.loadSecrets();
            expect(records).toHaveLength(1);
            expect(await vault.decryptSecret(records[0])).toBe(
                `isolated record ${index}`,
            );
        } finally {
            vault.close();
        }
    }
}, 30000);

it("creates duplicate names concurrently without replacing or adopting a vault", async () => {
    const name = `Concurrent ${crypto.randomUUID()}`;
    const vaults = await Promise.all([
        createVault("first-password", name),
        createVault("second-password", name),
    ]);
    expect(vaults[0].id).not.toBe(vaults[1].id);
    for (const [index, vault] of vaults.entries()) {
        await vault.encryptAndStore(`concurrent ${index}`);
        vault.close();
    }
    for (const [index, original] of vaults.entries()) {
        const vault = await unlockVault(
            index ? "second-password" : "first-password",
            original.id,
        );
        expect(await vault.decryptSecret((await vault.loadSecrets())[0])).toBe(
            `concurrent ${index}`,
        );
        vault.close();
    }
}, 15000);

it("does not start a vault when registry reservation fails", async () => {
    const before = await indexedDB.databases();
    vi.spyOn(IDBObjectStore.prototype, "add").mockImplementationOnce(() => {
        throw new DOMException("Full", "QuotaExceededError");
    });
    await expect(createVault("test-password", "Denied")).rejects.toThrow(
        "local vault registry",
    );
    expect(await indexedDB.databases()).toEqual(before);
});

it("recovers completed data after interrupted registration rather than claiming creation succeeded", async () => {
    const name = `Interrupted ${crypto.randomUUID()}`;
    const put = IDBObjectStore.prototype.put;
    vi.spyOn(IDBObjectStore.prototype, "put").mockImplementation(function (
        this: IDBObjectStore,
        ...args
    ) {
        if (this.name === "vaults")
            throw new DOMException("Full", "QuotaExceededError");
        return put.apply(this, args);
    });
    await expect(createVault("test-password", name)).rejects.toThrow(
        "local vault registry",
    );
    vi.restoreAllMocks();
    const entry = (await listVaults()).find((v) => v.name === name);
    if (!entry) throw new Error("Missing persisted vault identity");
    expect(entry.status).toBe("pending");
    const recovered = await unlockVault("test-password", entry.id);
    await recovered.encryptAndStore("recovered content");
    recovered.close();
    expect((await listVaults()).find((v) => v.id === entry.id)?.status).toBe(
        "ready",
    );
    const reopened = await unlockVault("test-password", entry.id);
    expect(
        await reopened.decryptSecret((await reopened.loadSecrets())[0]),
    ).toBe("recovered content");
    reopened.close();
}, 15000);

it("keeps an interrupted reservation visible without adopting a later same-name vault", async () => {
    const name = `Incomplete ${crypto.randomUUID()}`;
    vi.spyOn(Database, "open").mockRejectedValueOnce(
        new Error("Storage denied"),
    );
    await expect(createVault("first-password", name)).rejects.toThrow(
        "Storage denied",
    );
    vi.restoreAllMocks();
    const pending = (await listVaults()).find((v) => v.name === name);
    if (!pending) throw new Error("Missing persisted vault identity");
    expect(pending.status).toBe("pending");
    const other = await createVault("second-password", name);
    expect(other.id).not.toBe(pending.id);
    await other.encryptAndStore("other vault content");
    other.close();
    await expect(unlockVault("first-password", pending.id)).rejects.toThrow(
        "interrupted",
    );
    const reopened = await unlockVault("second-password", other.id);
    expect(
        await reopened.decryptSecret((await reopened.loadSecrets())[0]),
    ).toBe("other vault content");
    reopened.close();
}, 15000);

it("rejects a repeated identity reservation without changing the existing vault", async () => {
    const original = await createVault(
        "original-password",
        `Original ${crypto.randomUUID()}`,
    );
    await original.encryptAndStore("original content");
    const id = original.id;
    original.close();
    vi.spyOn(crypto, "randomUUID").mockReturnValue(
        id as `${string}-${string}-${string}-${string}-${string}`,
    );
    await expect(
        createVault("replacement-password", "Replacement"),
    ).rejects.toThrow("local vault registry");
    vi.restoreAllMocks();
    const reopened = await unlockVault("original-password", id);
    expect(
        await reopened.decryptSecret((await reopened.loadSecrets())[0]),
    ).toBe("original content");
    reopened.close();
}, 15000);

it("does not unlock storage whose metadata identity differs from the registry", async () => {
    const vault = await createVault(
        "test-password",
        `Mismatch ${crypto.randomUUID()}`,
    );
    const id = vault.id;
    vault.close();
    const entry = (await listVaults()).find((v) => v.id === id);
    if (!entry) throw new Error("Missing persisted vault identity");
    const db = await Database.open(entry.databaseName);
    const client = db.client();
    try {
        await client.execute(
            "UPDATE vault_meta SET vault_id = $1 WHERE id = 1",
            [{ type: "text", value: crypto.randomUUID() }],
        );
        await expect(unlockVault("test-password", id)).rejects.toThrow(
            "identity does not match",
        );
        expect(
            (
                await client.execute("SELECT salt FROM vault_meta WHERE id = 1")
            )[0].rows,
        ).toHaveLength(1);
    } finally {
        client.close();
        db.close();
    }
}, 15000);

async function legacyVault() {
    const name = `legacy-${crypto.randomUUID()}`;
    const id = crypto.randomUUID();
    const salt = crypto.getRandomValues(new Uint8Array(16));
    const encode = (data: Uint8Array) => btoa(String.fromCharCode(...data));
    const material = await crypto.subtle.importKey(
        "raw",
        new TextEncoder().encode("legacy-password"),
        "PBKDF2",
        false,
        ["deriveKey"],
    );
    const key = await crypto.subtle.deriveKey(
        { name: "PBKDF2", salt, iterations: 600000, hash: "SHA-512" },
        material,
        { name: "AES-GCM", length: 256 },
        false,
        ["encrypt"],
    );
    const nonce = crypto.getRandomValues(new Uint8Array(12));
    const ciphertext = new Uint8Array(
        await crypto.subtle.encrypt(
            { name: "AES-GCM", iv: nonce },
            key,
            new TextEncoder().encode("existing legacy record"),
        ),
    );
    const db = await Database.open(`password-manager-vault-${name}`);
    const client = db.client();
    await client.migrate(vaultMigrations);
    await client.execute(
        "INSERT INTO vault_meta (id, vault_id, salt, iv, created_at) VALUES (1, $1, $2, $3, $4)",
        [
            { type: "text", value: id },
            { type: "text", value: encode(salt) },
            { type: "text", value: encode(nonce) },
            { type: "text", value: "2026-01-01" },
        ],
    );
    await client.execute(
        "INSERT INTO secrets (id, ciphertext, nonce, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)",
        [
            { type: "text", value: "legacy-record" },
            { type: "text", value: encode(ciphertext) },
            { type: "text", value: encode(nonce) },
            { type: "text", value: "2026-01-01" },
        ],
    );
    client.close();
    db.close();
    return { id, name };
}

it("registers existing v1 vaults in place and preserves records alongside colliding new names", async () => {
    const legacy = await legacyVault();
    const entries = await listVaults();
    expect(entries).toContainEqual(
        expect.objectContaining({ ...legacy, status: "ready" }),
    );
    const newer = await createVault("new-password", legacy.name.toUpperCase());
    expect(newer.id).not.toBe(legacy.id);
    newer.close();
    const vault = await unlockVault("legacy-password", legacy.id);
    expect(await vault.decryptSecret((await vault.loadSecrets())[0])).toBe(
        "existing legacy record",
    );
    vault.close();
    const reopened = await unlockVault("legacy-password", legacy.id);
    expect((await reopened.loadSecrets())[0].id).toBe("legacy-record");
    reopened.close();
}, 15000);

it("registers an existing vault concurrently without replacing its identity", async () => {
    const legacy = await legacyVault();
    const results = await Promise.all([listVaults(), listVaults()]);
    for (const result of results) {
        expect(result.filter((v) => v.id === legacy.id)).toEqual([
            expect.objectContaining(legacy),
        ]);
    }
    const vault = await unlockVault("legacy-password", legacy.id);
    expect(await vault.decryptSecret((await vault.loadSecrets())[0])).toBe(
        "existing legacy record",
    );
    vault.close();
}, 15000);

it("leaves the existing source usable when its registry transition fails", async () => {
    const legacy = await legacyVault();
    vi.spyOn(IDBObjectStore.prototype, "add").mockImplementationOnce(() => {
        throw new DOMException("Denied", "QuotaExceededError");
    });
    await expect(listVaults()).rejects.toThrow("source vault is unchanged");
    vi.restoreAllMocks();
    expect(await listVaults()).toContainEqual(expect.objectContaining(legacy));
    const vault = await unlockVault("legacy-password", legacy.id);
    expect(await vault.decryptSecret((await vault.loadSecrets())[0])).toBe(
        "existing legacy record",
    );
    vault.close();
}, 15000);
