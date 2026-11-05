import { Database, MigrationError } from "@ofsdk/sql";
import { expect, it } from "vitest";
import { vaultMigrations } from "./migrations";
import { createVault, unlockVault, vaultExists } from "./vault";

it("authenticates an empty vault after close and reopen", async () => {
    const name = `test-empty-${crypto.randomUUID()}`;
    const vault = await createVault("original-password", name);
    vault.close();
    await expect(unlockVault("wrong-password", name)).rejects.toThrow(
        "Wrong password",
    );
    const reopened = await unlockVault("original-password", name);
    try {
        expect(await reopened.loadSecrets()).toEqual([]);
    } finally {
        reopened.close();
    }
}, 15000);

it("rejects an old development schema without migration history", async () => {
    const name = `test-old-${crypto.randomUUID()}`;
    const db = await Database.open(`password-manager-vault-${name}`);
    const client = db.client();
    try {
        await client.execute(
            "CREATE TABLE vault_meta (id TEXT PRIMARY KEY, salt TEXT NOT NULL, iv TEXT NOT NULL, created_at TEXT NOT NULL); CREATE TABLE secrets (id TEXT PRIMARY KEY, ciphertext TEXT NOT NULL, nonce TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
        );
        await expect(
            createVault("test-only-master-password", name),
        ).rejects.toBeInstanceOf(MigrationError);
        await expect(
            unlockVault("test-only-master-password", name),
        ).rejects.toBeInstanceOf(MigrationError);
    } finally {
        client.close();
        db.close();
    }
}, 15000);

it("creates a vault in a migrated empty database without repeating schema changes", async () => {
    const name = `test-prepared-${crypto.randomUUID()}`;
    const db = await Database.open(`password-manager-vault-${name}`);
    const client = db.client();
    try {
        expect(await client.migrate(vaultMigrations)).toEqual({
            currentVersion: 2,
            appliedVersions: [1, 2],
        });
        expect(await vaultExists(name)).toBe(false);
        const vault = await createVault("test-only-master-password", name);
        vault.close();
        expect(await vaultExists(name)).toBe(true);
        expect(await client.migrate(vaultMigrations)).toEqual({
            currentVersion: 2,
            appliedVersions: [],
        });
    } finally {
        client.close();
        db.close();
    }
}, 15000);

it("rejects duplicate creation without changing the original vault", async () => {
    const name = `test-duplicate-${crypto.randomUUID()}`;
    const vault = await createVault("original-password", name);
    const vaultId = vault.id;
    let secretId: string;
    try {
        secretId = (await vault.encryptAndStore("original encrypted content"))
            .id;
        await expect(
            createVault("replacement-password", name),
        ).rejects.toThrow();
        expect(await vault.loadSecrets()).toHaveLength(1);
    } finally {
        vault.close();
    }
    const reopened = await unlockVault("original-password", name);
    try {
        expect(reopened.id).toBe(vaultId);
        const [secret] = await reopened.loadSecrets();
        expect(secret.id).toBe(secretId);
        expect(await reopened.decryptSecret(secret)).toBe(
            "original encrypted content",
        );
    } finally {
        reopened.close();
    }
}, 15000);

it("rejects a newer schema before creating or unlocking a vault", async () => {
    const name = `test-newer-${crypto.randomUUID()}`;
    const vault = await createVault("test-only-master-password", name);
    vault.close();
    const db = await Database.open(`password-manager-vault-${name}`);
    const client = db.client();
    try {
        await client.migrate([
            ...vaultMigrations,
            {
                version: 3,
                sql: "ALTER TABLE secrets ADD COLUMN future_note TEXT",
            },
        ]);
        for (const open of [createVault, unlockVault]) {
            await expect(
                open("test-only-master-password", name),
            ).rejects.toMatchObject({
                name: "MigrationError",
                phase: "History",
                version: 3,
            });
        }
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
    const unlocked = await unlockVault(password, name);
    try {
        expect(
            await unlocked.decryptSecret((await unlocked.loadSecrets())[0]),
        ).toBe(plaintext);
        await unlocked.deleteSecret(id);
        expect(await unlocked.loadSecrets()).toEqual([]);
    } finally {
        unlocked.close();
    }
    const reopened = await unlockVault(password, name);
    try {
        expect(await reopened.loadSecrets()).toEqual([]);
    } finally {
        reopened.close();
    }
}, 15000);
