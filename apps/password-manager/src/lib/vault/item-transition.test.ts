import { Database } from "@ofsdk/sql";
import { flushSync, mount, tick, unmount } from "svelte";
import * as v from "valibot";
import { afterEach, expect, it, vi } from "vitest";
import Page from "../../routes/+page.svelte";
import { SecretSchema } from "../secrets/secret";
import { prepareBackupRestore } from "./backup";
import { createVault, listVaults, unlockVault } from "./vault";

const password = "synthetic-transition-password";
const date = "2026-01-02T03:04:05.000Z";
const login = {
    id: "12345678-1234-4234-8234-123456789abc",
    name: "Legacy 日本語",
    secret: "PRIVATE-TRANSITION",
    username: "0001",
    totp: "seed",
    notes: "a\nb",
    uris: [
        { uri: "androidapp://synthetic", match: "never" },
        { uri: "example.com" },
    ],
    fields: [
        { name: "PIN", type: "hidden", value: "0000" },
        { name: "Enabled", type: "boolean", value: "false" },
    ],
    favorite: true,
    history: [{ secret: "previous", changedAt: date }],
    createdAt: date,
    updatedAt: date,
    deletedAt: date,
};
const transitioned = {
    ...login,
    kind: "login",
    uris: [login.uris[0], { uri: "example.com", match: "domain" }],
};

async function source(values: unknown[] = [login]) {
    const name = `Transition ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    for (const [index, value] of values.entries())
        await vault.encryptAndStore(
            JSON.stringify(value),
            index === 0 ? login.id : "12345678-1234-4234-8234-123456789abd",
        );
    const identity = (await listVaults()).find(
        (value) => value.id === vault.id,
    );
    if (!identity) throw new Error("Missing test identity");
    const db = await Database.open(identity.databaseName);
    const client = db.client();
    await client.execute("UPDATE vault_meta SET item_format = 1 WHERE id = 1");
    client.close();
    db.close();
    return { vault, identity };
}
async function raw(id: string, databaseName?: string) {
    const identity = (await listVaults()).find((value) => value.id === id);
    if (!identity) throw new Error("Missing test identity");
    const db = await Database.open(databaseName ?? identity.databaseName);
    const client = db.client();
    try {
        return await client.execute(
            "SELECT * FROM vault_meta; SELECT * FROM secrets ORDER BY id",
        );
    } finally {
        client.close();
        db.close();
    }
}
async function clearStorage() {
    for (const { name } of await indexedDB.databases()) {
        if (!name) continue;
        await new Promise<void>((resolve, reject) => {
            const request = indexedDB.deleteDatabase(name);
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
            request.onblocked = () =>
                reject(new Error("Test storage is still open"));
        });
    }
}
afterEach(() => vi.restoreAllMocks());

it("transitions the complete legacy snapshot and restores both current and pre-transition backups in clean storage", async () => {
    const { vault, identity } = await source();
    const id = vault.id;
    const originalStorage = await raw(id);
    const before = await vault.loadSecrets();
    await vault.prepareItems();
    const records = await vault.loadSecrets();
    expect(await raw(id, identity.databaseName)).toEqual(originalStorage);
    expect(records[0].createdAt).toBe(before[0].createdAt);
    expect(records[0].updatedAt).toBe(before[0].updatedAt);
    expect(JSON.parse(await vault.decryptSecret(records[0]))).toEqual(
        transitioned,
    );
    const backup = await vault.exportTransitionBackup();
    const current = await vault.exportBackup();
    expect(JSON.parse(backup).version).toBe(1);
    expect(JSON.parse(current).version).toBe(2);
    expect(backup + current).not.toContain(login.secret);
    vault.close();
    const reopened = await unlockVault(password, id);
    await reopened.prepareItems();
    expect(await reopened.loadSecrets()).toEqual(records);
    expect(await reopened.exportTransitionBackup()).toBe(backup);
    reopened.close();
    await clearStorage();
    await (await prepareBackupRestore(password, current)).restore();
    const restored = await unlockVault(password, id);
    await restored.prepareItems();
    expect(await restored.loadSecrets()).toEqual(records);
    expect(await restored.exportTransitionBackup()).toBe(backup);
    restored.close();
    await clearStorage();
    await (await prepareBackupRestore(password, backup)).restore();
    const original = await unlockVault(password, id);
    expect(await original.loadSecrets()).toEqual(before);
    await original.prepareItems();
    expect(
        JSON.parse(
            await original.decryptSecret((await original.loadSecrets())[0]),
        ),
    ).toEqual(transitioned);
    original.close();
}, 15000);

it.each([
    {
        ...login,
        id: "12345678-1234-4234-8234-123456789abd",
        uris: [{ uri: "valid", match: "unknown" }],
    },
    {
        ...login,
        id: "12345678-1234-4234-8234-123456789abd",
        fields: [
            { name: "Hidden", type: "linked", value: "PRIVATE-TRANSITION" },
        ],
    },
    {
        ...login,
        id: "12345678-1234-4234-8234-123456789abd",
        extra: "PRIVATE-TRANSITION",
    },
    { ...login, id: login.id },
    {
        ...login,
        id: "12345678-1234-4234-8234-123456789abd",
        history: [{ secret: "PRIVATE-TRANSITION", changedAt: "invalid" }],
    },
])(
    "rejects a malformed later legacy record without writing or exposing a partial snapshot (%#)",
    async (bad) => {
        const { vault } = await source([login, bad]);
        const before = await raw(vault.id);
        try {
            await expect(vault.prepareItems()).rejects.toThrow(
                "No partial records",
            );
            expect(await raw(vault.id)).toEqual(before);
            await expect(vault.exportBackup()).rejects.toThrow(
                "No source data was changed",
            );
        } finally {
            vault.close();
        }
    },
    15000,
);

it.each(["quota", "lost", "uncertain"])(
    "keeps a recoverable complete source after a %s transition write",
    async (failure) => {
        const { vault } = await source([
            login,
            {
                ...login,
                id: "12345678-1234-4234-8234-123456789abd",
                name: "Second",
            },
        ]);
        const id = vault.id;
        const before = await raw(id);
        const put = IDBObjectStore.prototype.put;
        let injected = false;
        let schemaSaved = false;
        const fault = vi
            .spyOn(IDBObjectStore.prototype, "put")
            .mockImplementation(function (this: IDBObjectStore, value, key) {
                if (this.name === "meta" && key === "revision" && value === "2")
                    schemaSaved = true;
                if (!schemaSaved || this.name !== "records")
                    return put.call(this, value, key);
                injected = true;
                if (failure === "quota")
                    throw new DOMException(
                        "Synthetic full storage",
                        "QuotaExceededError",
                    );
                if (failure === "lost")
                    return put.call(this, value, "synthetic-lost-transition");
                const request = put.call(this, value, key);
                // The durable request succeeds, but the next storage reopen cannot be confirmed.
                vi.spyOn(Database, "open").mockRejectedValueOnce(
                    new Error("Synthetic reopen failure"),
                );
                return request;
            });
        await expect(vault.prepareItems()).rejects.toThrow(
            "could not be confirmed",
        );
        expect(injected).toBe(true);
        fault.mockRestore();
        vi.restoreAllMocks();
        vault.close();
        if (failure !== "uncertain") expect(await raw(id)).toEqual(before);
        const reopened = await unlockVault(password, id);
        await reopened.prepareItems();
        const records = await reopened.loadSecrets();
        expect(records).toHaveLength(2);
        expect(JSON.parse(await reopened.decryptSecret(records[0]))).toEqual(
            transitioned,
        );
        expect(await reopened.exportTransitionBackup()).not.toContain(
            login.secret,
        );
        reopened.close();
    },
    15000,
);

it("serializes competing transitions, rejects stale handles after close, and preserves one recovery snapshot", async () => {
    const { vault } = await source();
    const second = await unlockVault(password, vault.id);
    await Promise.all([vault.prepareItems(), second.prepareItems()]);
    expect(await second.loadSecrets()).toEqual(await vault.loadSecrets());
    expect(await second.exportTransitionBackup()).toBe(
        await vault.exportTransitionBackup(),
    );
    const record = (await vault.loadSecrets())[0];
    vault.close();
    await expect(vault.decryptSecret(record)).rejects.toThrow("closed");
    await expect(vault.prepareItems()).rejects.toThrow("closed");
    await expect(
        vault.encryptAndStore("PRIVATE-TRANSITION", record.id),
    ).rejects.toThrow("closed");
    await expect(vault.deleteSecret(record.id)).rejects.toThrow("closed");
    await expect(vault.exportBackup()).rejects.toThrow("closed");
    second.close();
}, 15000);

it("persists all five exact field sets, optional values, inherited rules and encrypted provenance through backup/restore", async () => {
    const vault = await createVault(password, "Five typed records");
    await vault.prepareItems();
    const common = {
        name: "Synthetic",
        createdAt: date,
        updatedAt: date,
        notes: "private\n日本語",
        favorite: false,
        fields: [],
        history: [],
        folderId: crypto.randomUUID(),
        importSource: { format: "personal-json-v1", id: "external-source-id" },
    };
    const values = [
        {
            kind: "login",
            username: null,
            secret: "",
            totp: "",
            uris: [
                { uri: "scheme-less", match: null },
                { uri: "regex.*", match: "regex" },
            ],
        },
        {
            kind: "card",
            cardholderName: "A",
            brand: "Unlisted",
            number: "00001",
            expMonth: "00",
            expYear: "1900",
            code: "001",
        },
        {
            kind: "identity",
            title: "Localized",
            firstName: "A",
            middleName: "B",
            lastName: "C",
            address1: "1",
            address2: "2",
            address3: "3",
            city: "City",
            state: "State",
            postalCode: "00001",
            country: "Unlisted",
            company: "Co",
            email: "original",
            phone: "+001",
            ssn: "00001",
            username: "User",
            passportNumber: "001",
            licenseNumber: "002",
        },
        { kind: "secureNote", type: 0 },
        {
            kind: "sshKey",
            privateKey: "  synthetic\nprivate text\n",
            publicKey: "public",
            keyFingerprint: "original fingerprint",
        },
    ].map((value) =>
        v.parse(SecretSchema, { ...common, ...value, id: crypto.randomUUID() }),
    );
    for (const value of values)
        await vault.encryptAndStore(JSON.stringify(value), value.id);
    const id = vault.id;
    const backup = await vault.exportBackup();
    expect(backup).not.toContain("external-source-id");
    const before = await vault.loadSecrets();
    vault.close();
    await clearStorage();
    await (await prepareBackupRestore(password, backup)).restore();
    const reopened = await unlockVault(password, id);
    await reopened.prepareItems();
    const saved = await reopened.loadSecrets();
    expect(saved).toEqual(before);
    for (const value of values) {
        const record = saved.find((entry) => entry.id === value.id);
        if (!record) throw new Error("Missing persisted item");
        expect(JSON.parse(await reopened.decryptSecret(record))).toEqual(value);
    }
    for (const bad of [
        { ...values[0], kind: "unknown" },
        { ...values[0], card: {} },
        { ...values[4], privateKey: " " },
        { ...values[3], type: 1 },
    ])
        await expect(
            reopened.encryptAndStore(JSON.stringify(bad), bad.id),
        ).rejects.toThrow("Invalid typed");
    expect(await reopened.loadSecrets()).toEqual(saved);
    reopened.close();
}, 15000);

it("mounts the real page to transition, cancel nested edits, save optional credentials, lock and reopen", async () => {
    const { vault, identity } = await source([
        { ...login, deletedAt: undefined },
    ]);
    vault.close();
    const target = document.createElement("div");
    document.body.append(target);
    const component = mount(Page, { target });
    flushSync();
    const button = (name: string) => {
        const found = [...target.querySelectorAll("button")].find(
            (element) =>
                (
                    element.querySelector(".vault-name")?.textContent ??
                    element.textContent
                )?.trim() === name,
        );
        if (!found) throw new Error(`Missing button: ${name}`);
        return found;
    };
    const click = async (name: string) => {
        expect(button(name).disabled).toBe(false);
        button(name).click();
        flushSync();
        await tick();
    };
    const field = (id: string) => {
        const input = target.querySelector<
            HTMLInputElement | HTMLSelectElement
        >(`#${id}`);
        if (!input) throw new Error(`Missing field: ${id}`);
        return input;
    };
    const fill = (id: string, value: string) => {
        const input = field(id);
        input.value = value;
        input.dispatchEvent(
            new Event(input instanceof HTMLSelectElement ? "change" : "input", {
                bubbles: true,
            }),
        );
        flushSync();
    };
    try {
        await expect.poll(() => target.textContent).toContain(identity.name);
        await click(identity.name);
        fill("password", password);
        const put = IDBObjectStore.prototype.put;
        let schemaSaved = false;
        const fault = vi
            .spyOn(IDBObjectStore.prototype, "put")
            .mockImplementation(function (this: IDBObjectStore, value, key) {
                if (this.name === "meta" && key === "revision" && value === "2")
                    schemaSaved = true;
                if (schemaSaved && this.name === "records")
                    throw new DOMException(
                        "Synthetic full storage",
                        "QuotaExceededError",
                    );
                return put.call(this, value, key);
            });
        await click("Unlock");
        await expect
            .poll(() => target.textContent)
            .toContain("Vault unavailable");
        expect(target.textContent).not.toContain(login.name);
        expect(button("New secret").disabled).toBe(true);
        expect(target.textContent).not.toContain("Saved.");
        fault.mockRestore();
        let download: Blob | undefined;
        vi.spyOn(URL, "createObjectURL").mockImplementation((blob) => {
            download = blob as Blob;
            return "blob:synthetic-transition-backup";
        });
        vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(
            () => {},
        );
        await click("Download encrypted backup");
        await expect.poll(() => download).toBeDefined();
        if (!download) throw new Error("Missing original source download");
        expect(JSON.parse(await download.text()).version).toBe(1);
        await expect.poll(() => button("Lock vault").disabled).toBe(false);
        await click("Refresh");
        await expect.poll(() => target.textContent).toContain(login.name);
        download = undefined;
        await click("Download pre-transition backup");
        await expect.poll(() => download).toBeDefined();
        if (!download) throw new Error("Missing pre-transition download");
        expect(
            (
                await prepareBackupRestore(
                    password,
                    await (download as Blob).text(),
                )
            ).recordCount,
        ).toBe(1);
        await expect.poll(() => button("Lock vault").disabled).toBe(false);
        const persisted = await raw(identity.id);
        await click(`★ ${login.name}`);
        await click("Edit");
        fill("uri-0", "discarded");
        fill("match-1", "never");
        fill("field-value-0", "discarded");
        await click("Add website");
        await click("Cancel");
        expect(await raw(identity.id)).toEqual(persisted);
        await click(`★ ${login.name}`);
        await click("Edit");
        expect(field("uri-0").value).toBe("androidapp://synthetic");
        expect(field("field-value-0").value).toBe("0000");
        fill("secret", "");
        await click("Save");
        await expect.poll(() => target.textContent).toContain("Saved.");
        for (const [name, username] of [
            ["Username only", "alice"],
            ["No credentials", ""],
        ]) {
            await click("New secret");
            fill("name", name);
            fill("username", username);
            expect(target.querySelector("#confirm-secret")).toBeNull();
            await click("Save");
            await expect.poll(() => target.textContent).toContain("Saved.");
        }
        await click("Lock vault");
        expect(target.querySelector("#secret")).toBeNull();
        await expect.poll(() => target.textContent).toContain(identity.name);
        await click(identity.name);
        fill("password", password);
        await click("Unlock");
        await expect.poll(() => target.textContent).toContain("Username only");
        await click("Username only");
        await click("Edit");
        expect(field("username").value).toBe("alice");
        expect(field("secret").value).toBe("");
        await click("Cancel");
        await click(`★ ${login.name}`);
        await click("Edit");
        expect(field("secret").value).toBe("");
        expect(field("totp").value).toBe("seed");
        expect(field("match-1").value).toBe("domain");
        await click("Cancel");
        await click("Lock vault");
        const saved = await unlockVault(password, identity.id);
        const items = await saved.loadSecrets();
        const previous = items.find((item) => item.id === login.id);
        if (!previous) throw new Error("Missing original item");
        const expected: Partial<typeof transitioned> = { ...transitioned };
        delete expected.deletedAt;
        expect(JSON.parse(await saved.decryptSecret(previous))).toMatchObject({
            ...expected,
            updatedAt: expect.any(String),
            secret: "",
            history: [
                login.history[0],
                { secret: login.secret, changedAt: expect.any(String) },
            ],
        });
        expect(
            JSON.parse(await saved.decryptSecret(previous)),
        ).not.toHaveProperty("deletedAt");
        saved.close();
    } finally {
        await unmount(component);
        target.remove();
    }
}, 20000);

it.each(["rejected", "unconfirmed"])(
    "retains the original database after %s transition publication and verifies retry",
    async (failure) => {
        const { vault, identity } = await source();
        const before = await raw(vault.id);
        const put = IDBObjectStore.prototype.put;
        let published = false;
        const fault = vi
            .spyOn(IDBObjectStore.prototype, "put")
            .mockImplementation(function (this: IDBObjectStore, value, key) {
                if (this.name === "vaults") {
                    if (failure === "rejected")
                        throw new DOMException(
                            "Synthetic registry failure",
                            "QuotaExceededError",
                        );
                    published = true;
                }
                return put.call(this, value, key);
            });
        const getAll = IDBObjectStore.prototype.getAll;
        const read = vi
            .spyOn(IDBObjectStore.prototype, "getAll")
            .mockImplementation(function (this: IDBObjectStore, ...args) {
                if (this.name === "vaults" && published)
                    throw new DOMException(
                        "Synthetic readback failure",
                        "UnknownError",
                    );
                return getAll.apply(this, args);
            });
        await expect(vault.prepareItems()).rejects.toThrow(
            "could not be confirmed",
        );
        fault.mockRestore();
        read.mockRestore();
        expect(await raw(vault.id, identity.databaseName)).toEqual(before);
        await vault.prepareItems();
        expect(
            JSON.parse(
                await vault.decryptSecret((await vault.loadSecrets())[0]),
            ),
        ).toEqual(transitioned);
        const current = (await listVaults()).find(
            (value) => value.id === vault.id,
        );
        expect(current?.previousDatabaseName).toBe(identity.databaseName);
        const snapshot = await vault.loadSecrets();
        await vault.prepareItems();
        expect(await vault.loadSecrets()).toEqual(snapshot);
        vault.close();
    },
    15000,
);

it("exports verified pre-transition recovery despite damaged current records and rejects damaged recovery metadata", async () => {
    const { vault, identity } = await source();
    const before = await raw(vault.id);
    await vault.prepareItems();
    const backup = await vault.exportTransitionBackup();
    const current = (await listVaults()).find((value) => value.id === vault.id);
    if (!current) throw new Error("Missing transitioned identity");
    const db = await Database.open(current.databaseName);
    const client = db.client();
    try {
        await client.execute("UPDATE secrets SET ciphertext = 'AA=='");
        await expect(vault.exportBackup()).rejects.toThrow(
            "Cannot create a complete",
        );
        expect(await vault.exportTransitionBackup()).toBe(backup);
        expect((await prepareBackupRestore(password, backup)).recordCount).toBe(
            1,
        );
        await client.execute(
            "UPDATE vault_meta SET transition_backup = 'damaged'",
        );
        await expect(vault.exportTransitionBackup()).rejects.toThrow(
            "pre-transition backup",
        );
        expect(await raw(vault.id, identity.databaseName)).toEqual(before);
    } finally {
        client.close();
        db.close();
        vault.close();
    }
}, 15000);
