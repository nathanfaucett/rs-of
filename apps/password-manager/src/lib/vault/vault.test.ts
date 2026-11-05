import { Database } from "@ofsdk/sql";
import { expect, it } from "vitest";
import { vaultMigrations } from "./migrations";
import { createVault, listVaults, unlockVault } from "./vault";

it("does not overwrite an old development schema without migration history", async () => {
    const name = `test-old-${crypto.randomUUID()}`;
    const db = await Database.open(`password-manager-vault-${name}`);
    const client = db.client();
    try {
        await client.execute(
            "CREATE TABLE vault_meta (id TEXT PRIMARY KEY, salt TEXT NOT NULL, iv TEXT NOT NULL, created_at TEXT NOT NULL); CREATE TABLE secrets (id TEXT PRIMARY KEY, ciphertext TEXT NOT NULL, nonce TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
        );
        const vault = await createVault("test-only-master-password", name);
        vault.close();
        await expect(listVaults()).rejects.toThrow("source vault is unchanged");
        expect(
            (await client.execute("SELECT salt FROM vault_meta"))[0].rows,
        ).toEqual([]);
    } finally {
        client.close();
        db.close();
        await new Promise<void>((resolve, reject) => {
            const request = indexedDB.deleteDatabase(
                `ofdb:sql:password-manager-vault-${name}`,
            );
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
        });
    }
}, 15000);

it("creates a v1 migrated vault and reopens without repeating schema changes", async () => {
    const vault = await createVault(
        "test-only-master-password",
        `test-prepared-${crypto.randomUUID()}`,
    );
    const id = vault.id;
    vault.close();
    const entry = (await listVaults()).find((v) => v.id === id);
    if (!entry) throw new Error("Missing persisted vault identity");
    const db = await Database.open(entry.databaseName);
    const client = db.client();
    try {
        expect(await client.migrate(vaultMigrations)).toEqual({
            currentVersion: 1,
            appliedVersions: [],
        });
    } finally {
        client.close();
        db.close();
    }
    const reopened = await unlockVault("test-only-master-password", id);
    expect(reopened.id).toBe(id);
    reopened.close();
}, 15000);

it("rejects a newer schema before unlocking and keeps new creation isolated", async () => {
    const name = `test-newer-${crypto.randomUUID()}`;
    const vault = await createVault("test-only-master-password", name);
    const id = vault.id;
    vault.close();
    const entry = (await listVaults()).find((v) => v.id === id);
    if (!entry) throw new Error("Missing persisted vault identity");
    const db = await Database.open(entry.databaseName);
    const client = db.client();
    try {
        await client.migrate([
            ...vaultMigrations,
            {
                version: 2,
                sql: "ALTER TABLE secrets ADD COLUMN future_note TEXT",
            },
        ]);
        await expect(
            unlockVault("test-only-master-password", id),
        ).rejects.toMatchObject({
            name: "MigrationError",
            phase: "History",
            version: 2,
        });
        const separate = await createVault("another-password", name);
        expect(separate.id).not.toBe(id);
        separate.close();
    } finally {
        client.close();
        db.close();
    }
}, 15000);

it("persists encrypted logins across lock/unlock and permanently removes a deleted login", async () => {
    const name = `test-crud-${crypto.randomUUID()}`;
    const password = "test-only-master-password";
    const plaintext = JSON.stringify({
        name: "Test login",
        username: "test-user",
        secret: "test-password",
        uris: [
            { uri: "https://example.com", match: "domain" },
            { uri: "https://example.org", match: "exact" },
        ],
    });
    const vault = await createVault(password, name);
    const vaultId = vault.id;
    let id: string;
    try {
        const stored = await vault.encryptAndStore(plaintext);
        id = stored.id;
        expect(stored.ciphertext).not.toContain("test-password");
        expect(await vault.decryptSecret((await vault.loadSecrets())[0])).toBe(
            plaintext,
        );
    } finally {
        vault.close();
    }
    const unlocked = await unlockVault(password, vaultId);
    try {
        expect(
            await unlocked.decryptSecret((await unlocked.loadSecrets())[0]),
        ).toBe(plaintext);
        await unlocked.deleteSecret(id);
        expect(await unlocked.loadSecrets()).toEqual([]);
    } finally {
        unlocked.close();
    }
    const reopened = await unlockVault(password, vaultId);
    try {
        expect(await reopened.loadSecrets()).toEqual([]);
    } finally {
        reopened.close();
    }
}, 15000);
