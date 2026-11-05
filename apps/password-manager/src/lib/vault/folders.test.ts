import { Database } from "@ofsdk/sql";
import { afterEach, expect, it, vi } from "vitest";
import { prepareBackupRestore } from "./backup";
import { createVault, listVaults, unlockVault } from "./vault";
import type { Secret } from "../secrets/secret";

const password = "synthetic-folders-master";
const date = "2026-01-02T03:04:05.000Z";
function item(name: string): Secret {
    return {
        kind: "secureNote",
        type: 0,
        id: crypto.randomUUID(),
        name,
        notes: "private note",
        favorite: false,
        fields: [],
        history: [],
        createdAt: date,
        updatedAt: date,
    };
}
afterEach(() => vi.restoreAllMocks());

it("keeps literal duplicate folder identities, atomically unfiles only direct members, and restores the complete encrypted snapshot in clean storage", async () => {
    const vault = await createVault(password, "Synthetic folders");
    await vault.prepareItems();
    const parent = await vault.createFolder(" Private / 日本語 ");
    const child = await vault.createFolder(" Private / 日本語 /child");
    const duplicate = await vault.createFolder(parent.name);
    expect(duplicate.id).not.toBe(parent.id);
    const first = { ...item("First"), folderId: parent.id };
    const second = { ...item("Second"), folderId: parent.id, deletedAt: date };
    const third = { ...item("Child"), folderId: child.id };
    await vault.mutateItems({ upsert: [first, second, third] });
    await vault.renameFolder(parent.id, "Renamed");
    expect(
        (await vault.readItems()).folders.find((f) => f.id === child.id)?.name,
    ).toBe(child.name);
    await vault.deleteFolder(parent.id);
    const snapshot = await vault.readItems();
    expect(snapshot.items.find((s) => s.id === first.id)?.folderId).toBeNull();
    expect(snapshot.items.find((s) => s.id === second.id)).toEqual({
        ...second,
        folderId: null,
        updatedAt: snapshot.items.find((s) => s.id === second.id)?.updatedAt,
    });
    expect(snapshot.items.find((s) => s.id === third.id)).toEqual(third);
    expect(snapshot.folders).toEqual(
        expect.arrayContaining([child, duplicate]),
    );
    const encrypted = await vault.loadFolders();
    expect(JSON.stringify(encrypted)).not.toContain("日本語");
    const backup = await vault.exportBackup();
    expect(JSON.parse(backup).version).toBe(3);
    expect(backup).not.toContain("日本語");
    const id = vault.id;
    vault.close();
    for (const entry of await indexedDB.databases()) {
        if (!entry.name) continue;
        await new Promise<void>((resolve, reject) => {
            const request = indexedDB.deleteDatabase(entry.name!);
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
            request.onblocked = () => reject(new Error("Open test database"));
        });
    }
    const preview = await prepareBackupRestore(password, backup);
    await preview.restore();
    const restored = await unlockVault(password, id);
    expect(await restored.readItems()).toEqual(snapshot);
    expect(await restored.loadFolders()).toEqual(encrypted);
    restored.close();
}, 20000);

it.each(["quota", "lost"])(
    "never reports a partial folder delete as saved after a %s write",
    async (phase) => {
        const vault = await createVault(password, `Failure ${phase}`);
        const folder = await vault.createFolder("Private folder");
        const first = { ...item("One"), folderId: folder.id };
        const second = { ...item("Two"), folderId: folder.id };
        await vault.mutateItems({ upsert: [first, second] });
        const before = await vault.readItems();
        const put = IDBObjectStore.prototype.put;
        let faults = 0;
        const fault = vi
            .spyOn(IDBObjectStore.prototype, "put")
            .mockImplementation(function (this: IDBObjectStore, value, key) {
                if (this.name === "records") {
                    faults++;
                    if (phase === "quota")
                        throw new DOMException(
                            "Synthetic quota",
                            "QuotaExceededError",
                        );
                    return put.call(this, value, "synthetic-lost-write");
                }
                return put.call(this, value, key);
            });
        await expect(vault.deleteFolder(folder.id)).rejects.toThrow(
            /could not be (confirmed|verified)/,
        );
        fault.mockRestore();
        expect(faults).toBeGreaterThan(0);
        const id = vault.id;
        vault.close();
        const reopened = await unlockVault(password, id);
        const after = await reopened.readItems();
        expect(after).toEqual(before);
        await reopened.deleteFolder(folder.id);
        expect(
            (await reopened.readItems()).items.every(
                (entry) => entry.folderId === null,
            ),
        ).toBe(true);
        reopened.close();
    },
    20000,
);

it.each(["rejected", "unconfirmed"])(
    "keeps a complete authoritative snapshot after %s move publication",
    async (phase) => {
        const vault = await createVault(password, `Move ${phase}`);
        const folder = await vault.createFolder("Destination");
        const first = item("One");
        const second = item("Two");
        await vault.mutateItems({ upsert: [first, second] });
        const before = await vault.readItems();
        const put = IDBObjectStore.prototype.put;
        const getAll = IDBObjectStore.prototype.getAll;
        let published = false;
        const write = vi
            .spyOn(IDBObjectStore.prototype, "put")
            .mockImplementation(function (this: IDBObjectStore, value, key) {
                if (this.name === "vaults") {
                    if (phase === "rejected")
                        throw new DOMException(
                            "Synthetic publication failure",
                            "QuotaExceededError",
                        );
                    published = true;
                }
                return put.call(this, value, key);
            });
        const read = vi
            .spyOn(IDBObjectStore.prototype, "getAll")
            .mockImplementation(function (this: IDBObjectStore, ...args) {
                if (this.name === "vaults" && published)
                    throw new DOMException(
                        "Synthetic read failure",
                        "UnknownError",
                    );
                return getAll.apply(this, args);
            });
        await expect(
            vault.moveItems([first.id, second.id], folder.id),
        ).rejects.toThrow("could not be confirmed");
        write.mockRestore();
        read.mockRestore();
        const id = vault.id;
        vault.close();
        const reopened = await unlockVault(password, id);
        const snapshot = await reopened.readItems();
        expect(snapshot.folders).toEqual(before.folders);
        if (phase === "rejected") expect(snapshot).toEqual(before);
        else {
            expect(snapshot.items).toHaveLength(2);
            expect(
                snapshot.items.every((value) => value.folderId === folder.id),
            ).toBe(true);
        }
        await reopened.moveItems([first.id, second.id], null);
        expect(
            (await reopened.readItems()).items.every(
                (value) => value.folderId === null,
            ),
        ).toBe(true);
        reopened.close();
    },
    20000,
);

it("rejects the complete snapshot and backup when a later encrypted folder has the wrong identity", async () => {
    const vault = await createVault(password, "Damaged folders");
    await vault.createFolder("One");
    await vault.createFolder("Two");
    await vault.mutateItems({ upsert: [item("Must not load partially")] });
    const identity = (await listVaults()).find(
        (entry) => entry.id === vault.id,
    );
    if (!identity) throw new Error("Missing test identity");
    const records = await vault.loadFolders();
    const db = await Database.open(identity.databaseName);
    const client = db.client();
    try {
        await client.execute(
            "UPDATE folders SET ciphertext = $1, nonce = $2 WHERE id = $3",
            [
                { type: "text", value: records[0].ciphertext },
                { type: "text", value: records[0].nonce },
                { type: "text", value: records[1].id },
            ],
        );
    } finally {
        client.close();
        db.close();
    }
    await expect(vault.readItems()).rejects.toThrow(
        "No partial records or folders",
    );
    await expect(vault.exportBackup()).rejects.toThrow();
    await expect(vault.createFolder("Must not write")).rejects.toThrow(
        "No partial records or folders",
    );
    vault.close();
}, 15000);

it("retires a pending folder mutation on lock without publishing or returning decrypted data", async () => {
    const vault = await createVault(password, "Pending folder");
    await vault.prepareItems();
    const id = vault.id;
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
        release = resolve;
    });
    let started = false;
    const encrypt = crypto.subtle.encrypt.bind(crypto.subtle);
    const delayed = vi
        .spyOn(crypto.subtle, "encrypt")
        .mockImplementationOnce(async (...args) => {
            started = true;
            await gate;
            return encrypt(...args);
        });
    const pending = vault.createFolder("Must not return after lock");
    const rejected = expect(pending).rejects.toThrow("closed");
    await expect.poll(() => started).toBe(true);
    vault.close();
    release();
    await rejected;
    delayed.mockRestore();
    const reopened = await unlockVault(password, id);
    expect(await reopened.readItems()).toEqual({ items: [], folders: [] });
    reopened.close();
}, 15000);

it("rejects invalid batches before writing and serializes concurrent handles without losing folder changes", async () => {
    const vault = await createVault(password, "Atomic owner");
    const other = await unlockVault(password, vault.id);
    const [left, right] = await Promise.all([
        vault.createFolder("Same"),
        other.createFolder("Same"),
    ]);
    expect((await vault.readItems()).folders).toHaveLength(2);
    const before = await vault.readItems();
    await expect(
        vault.mutateItems({
            upsert: [{ ...item("Dangling"), folderId: crypto.randomUUID() }],
            folders: [{ id: crypto.randomUUID(), name: "Must not save" }],
        }),
    ).rejects.toThrow("Malformed vault snapshot");
    expect(await other.readItems()).toEqual(before);
    await expect(vault.createFolder(" \n\t ")).rejects.toThrow(
        "name is required",
    );
    const isolated = await createVault(password, "Other vault");
    expect((await isolated.readItems()).folders).toEqual([]);
    await expect(
        isolated.moveItems([item("Missing").id], left.id),
    ).rejects.toThrow("no longer exists");
    isolated.close();
    await vault.renameFolder(left.id, "Changed");
    await other.deleteFolder(right.id);
    expect((await vault.readItems()).folders).toEqual([
        { id: left.id, name: "Changed" },
    ]);
    other.close();
    vault.close();
    await expect(vault.readItems()).rejects.toThrow("closed");
    await expect(vault.createFolder("Late")).rejects.toThrow("closed");
}, 20000);
