import { Client, Database } from "@ofsdk/sql";
import { afterEach, expect, it, vi } from "vitest";
import { prepareBackupRestore } from "./backup";
import { createVault, listVaults, unlockVault } from "./vault";

const password = "synthetic-lock-master";
afterEach(() => vi.restoreAllMocks());

it("persists encrypted timeout settings through reopen and clean backup restore", async () => {
    const vault = await createVault(password, "Lock recovery");
    const id = vault.id;
    expect(await vault.loadLockSettings()).toEqual({ timeoutMinutes: 5 });
    await vault.saveLockSettings({ timeoutMinutes: 15 });
    await vault.prepareItems();
    const item = {
        kind: "login",
        id: crypto.randomUUID(),
        name: "Synthetic recovery Login",
        secret: "PRIVATE-RECOVERY",
        favorite: false,
        fields: [],
        history: [],
        uris: [],
        createdAt: "2026-01-02T03:04:05.000Z",
        updatedAt: "2026-01-02T03:04:05.000Z",
    };
    await vault.encryptAndStore(JSON.stringify(item), item.id);
    const backup = await vault.exportBackup();
    expect(backup).not.toContain(item.secret);
    expect(backup).not.toContain("timeoutMinutes");
    vault.close();
    const reopened = await unlockVault(password, id);
    expect(await reopened.loadLockSettings()).toEqual({ timeoutMinutes: 15 });
    reopened.close();
    const identity = (await listVaults()).find((entry) => entry.id === id)!;
    const db = await Database.open(identity.databaseName);
    const client = db.client();
    const raw = await client.execute(
        "SELECT lock_settings FROM vault_meta WHERE id = 1",
    );
    expect(JSON.stringify(raw)).not.toContain("timeoutMinutes");
    client.close();
    db.close();
    for (const { name } of await indexedDB.databases()) {
        if (!name) continue;
        await new Promise<void>((resolve, reject) => {
            const request = indexedDB.deleteDatabase(name);
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
            request.onblocked = () => reject(new Error("Open test database"));
        });
    }
    await (await prepareBackupRestore(password, backup)).restore();
    await expect(unlockVault("wrong-password", id)).rejects.toThrow(
        "Wrong password",
    );
    const restored = await unlockVault(password, id);
    expect(await restored.loadLockSettings()).toEqual({ timeoutMinutes: 15 });
    expect(
        JSON.parse(
            await restored.decryptSecret((await restored.loadSecrets())[0]),
        ),
    ).toEqual(item);
    restored.close();
}, 15000);

it.each(["lost", "rejected-after-commit"] as const)(
    "does not acknowledge a %s settings write and reloads the authoritative value",
    async (failure) => {
        const vault = await createVault(password, "Uncertain settings");
        const execute = Client.prototype.execute;
        vi.spyOn(Client.prototype, "execute").mockImplementation(
            async function (...args) {
                if (!args[0].startsWith("UPDATE vault_meta SET lock_settings"))
                    return execute.apply(this, args);
                if (failure === "lost") return [];
                await execute.apply(this, args);
                throw new Error("Synthetic missing acknowledgement");
            },
        );
        await expect(
            vault.saveLockSettings({ timeoutMinutes: 30 }),
        ).rejects.toThrow("may have persisted");
        vault.close();
        vi.restoreAllMocks();
        const reopened = await unlockVault(password, vault.id);
        expect(await reopened.loadLockSettings()).toEqual({
            timeoutMinutes: failure === "lost" ? 5 : 30,
        });
        reopened.close();
    },
    15000,
);

it("does not reset damaged encrypted settings or export an unrecoverable backup", async () => {
    const vault = await createVault(password, "Damaged settings");
    await vault.saveLockSettings({ timeoutMinutes: 15 });
    vault.close();
    const identity = (await listVaults()).find(
        (entry) => entry.id === vault.id,
    )!;
    const db = await Database.open(identity.databaseName);
    const client = db.client();
    await client.execute(
        "UPDATE vault_meta SET lock_settings = $1 WHERE id = 1",
        [{ type: "text", value: "damaged-settings" }],
    );
    client.close();
    db.close();
    const reopened = await unlockVault(password, vault.id);
    await expect(reopened.loadLockSettings()).rejects.toThrow(
        "No settings were reset",
    );
    await expect(reopened.exportBackup()).rejects.toThrow();
    reopened.close();
}, 15000);

it("denies all operations through a retired handle, including preparation and delayed decryption", async () => {
    const vault = await createVault(password, "Retired lock handle");

    vault.close();
    const handle = await unlockVault(password, vault.id);
    const record = await handle.encryptAndStore("synthetic-private-value");
    const decrypt = crypto.subtle.decrypt.bind(crypto.subtle);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
        release = resolve;
    });
    vi.spyOn(crypto.subtle, "decrypt").mockImplementationOnce(
        async (...args) => {
            await gate;
            return decrypt(...args);
        },
    );
    const pending = handle.decryptSecret(record);
    handle.close();
    handle.close();
    release();
    await expect(pending).rejects.toThrow("closed");
    vi.restoreAllMocks();
    for (const operation of [
        () => handle.loadSecrets(),
        () => handle.decryptSecret(record),
        () => handle.encryptAndStore("never saved"),
        () => handle.deleteSecret(record.id),
        () => handle.prepareItems(),
        () => handle.exportBackup(),
        () => handle.exportTransitionBackup(),
        () => handle.loadLockSettings(),
        () => handle.saveLockSettings({ timeoutMinutes: 1 }),
    ])
        await expect(operation()).rejects.toThrow("closed");
}, 15000);

it.each(["item", "settings", "delete"] as const)(
    "rejects a late %s write acknowledgement after close and reloads the real committed outcome",
    async (operation) => {
        const vault = await createVault(password, `Pending ${operation}`);
        const original = await vault.encryptAndStore("synthetic-before");
        const execute = Client.prototype.execute;
        let release!: () => void;
        let sent!: () => void;
        const gate = new Promise<void>((resolve) => {
            release = resolve;
        });
        const started = new Promise<void>((resolve) => {
            sent = resolve;
        });
        const statement =
            operation === "settings"
                ? "UPDATE vault_meta SET lock_settings"
                : operation === "delete"
                  ? "DELETE FROM secrets"
                  : "INSERT INTO secrets";
        vi.spyOn(Client.prototype, "execute").mockImplementation(
            async function (...args) {
                const result = await execute.apply(this, args);
                if (args[0].startsWith(statement)) {
                    sent();
                    await gate;
                }
                return result;
            },
        );
        const pending =
            operation === "settings"
                ? vault.saveLockSettings({ timeoutMinutes: 30 })
                : operation === "delete"
                  ? vault.deleteSecret(original.id)
                  : vault.encryptAndStore("synthetic-after", original.id);
        const rejected = expect(pending).rejects.toThrow("closed");
        await started;
        vault.close();
        release();
        await rejected;
        vi.restoreAllMocks();
        const reopened = await unlockVault(password, vault.id);
        try {
            if (operation === "settings")
                expect(await reopened.loadLockSettings()).toEqual({
                    timeoutMinutes: 30,
                });
            else if (operation === "delete")
                expect(await reopened.loadSecrets()).toEqual([]);
            else
                expect(
                    await reopened.decryptSecret(
                        (await reopened.loadSecrets())[0],
                    ),
                ).toBe("synthetic-after");
        } finally {
            reopened.close();
        }
    },
    15000,
);

it("does not return a backup whose encryption completes after close", async () => {
    const vault = await createVault(password, "Late encrypted backup");
    let release!: () => void;
    let sent!: () => void;
    const gate = new Promise<void>((resolve) => {
        release = resolve;
    });
    const started = new Promise<void>((resolve) => {
        sent = resolve;
    });
    const encrypt = crypto.subtle.encrypt.bind(crypto.subtle);
    vi.spyOn(crypto.subtle, "encrypt").mockImplementationOnce(
        async (...args) => {
            sent();
            await gate;
            return encrypt(...args);
        },
    );
    const pending = vault.exportBackup();
    const rejected = expect(pending).rejects.toThrow("closed");
    await started;
    vault.close();
    release();
    await rejected;
}, 15000);
