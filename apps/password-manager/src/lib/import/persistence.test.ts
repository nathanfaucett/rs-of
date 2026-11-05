import { afterEach, expect, it, vi } from "vitest";
import supported from "../../../../../docs/fixtures/password-manager/supported.json";
import { prepareBackupRestore } from "$lib/vault/backup";
import { createVault, unlockVault } from "$lib/vault/vault";
import { preparePersonalJsonImport } from "./import";

const password = "synthetic-import-persistence";
const date = "2025-01-01T00:00:00.000Z";
afterEach(() => vi.restoreAllMocks());

it("retains existing folders, settings, Trash and history and restores imported ciphertext/provenance in clean storage", async () => {
    const vault = await createVault(
        password,
        `Import backup ${crypto.randomUUID()}`,
    );
    await vault.saveLockSettings({ timeoutMinutes: 15 });
    const folder = await vault.createFolder("Existing empty folder");
    const trashed = {
        kind: "secureNote" as const,
        type: 0 as const,
        id: crypto.randomUUID(),
        name: "Existing trash",
        notes: "SYNTHETIC-EXISTING",
        favorite: false,
        fields: [],
        history: [{ secret: "SYNTHETIC-OLD", changedAt: date }],
        folderId: folder.id,
        createdAt: date,
        updatedAt: date,
        deletedAt: date,
    };
    await vault.mutateItems({ upsert: [trashed] });
    const preview = await preparePersonalJsonImport(
        vault,
        JSON.stringify(supported),
    );
    const mutate = vi.spyOn(vault, "mutateItems");
    await preview.commit();
    expect(mutate).toHaveBeenCalledTimes(1);
    const snapshot = await vault.readItems();
    expect(snapshot.items.find((item) => item.id === trashed.id)).toEqual(
        trashed,
    );
    expect(snapshot.folders).toContainEqual(folder);
    expect(await vault.loadLockSettings()).toEqual({ timeoutMinutes: 15 });
    const records = await vault.loadSecrets();
    const encryptedFolders = await vault.loadFolders();
    const backup = await vault.exportBackup();
    for (const marker of [
        "SYNTHETIC-NOT-A-CREDENTIAL",
        "SYNTHETIC-PRIOR-ONE",
        "SYNTHETIC-NOT-AN-SSN",
        "SYNTHETIC PRIVATE KEY",
        supported.items[0].id,
        "Existing empty folder",
    ]) {
        expect(backup).not.toContain(marker);
        expect(JSON.stringify(records)).not.toContain(marker);
        expect(JSON.stringify(encryptedFolders)).not.toContain(marker);
    }
    const id = vault.id;
    vault.close();
    for (const entry of await indexedDB.databases()) {
        if (!entry.name) continue;
        await new Promise<void>((resolve, reject) => {
            const request = indexedDB.deleteDatabase(entry.name!);
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
            request.onblocked = () =>
                reject(new Error("Open synthetic database"));
        });
    }
    const recovery = await prepareBackupRestore(password, backup);
    await recovery.restore();
    const restored = await unlockVault(password, id);
    expect(await restored.readItems()).toEqual(snapshot);
    expect(await restored.loadSecrets()).toEqual(records);
    expect(await restored.loadFolders()).toEqual(encryptedFolders);
    expect(await restored.loadLockSettings()).toEqual({ timeoutMinutes: 15 });
    const repeat = await preparePersonalJsonImport(
        restored,
        JSON.stringify(supported),
    );
    expect(repeat.duplicates).toEqual({ sourceIds: 5, logical: 5 });
    repeat.retire();
    restored.close();
}, 20000);

it.each(["quota", "lost", "publication", "uncertain"])(
    "never publishes a partial import after %s failure",
    async (phase) => {
        const vault = await createVault(
            password,
            `Import fault ${phase} ${crypto.randomUUID()}`,
        );
        const folder = await vault.createFolder("Retained");
        const before = await vault.readItems();
        const preview = await preparePersonalJsonImport(
            vault,
            JSON.stringify(supported),
        );
        const put = IDBObjectStore.prototype.put;
        const getAll = IDBObjectStore.prototype.getAll;
        let published = false;
        let faults = 0;
        const write = vi
            .spyOn(IDBObjectStore.prototype, "put")
            .mockImplementation(function (this: IDBObjectStore, value, key) {
                if (
                    this.name === "records" &&
                    ["quota", "lost"].includes(phase)
                ) {
                    faults++;
                    if (phase === "quota")
                        throw new DOMException(
                            "synthetic",
                            "QuotaExceededError",
                        );
                    return put.call(this, value, "synthetic-lost-record");
                }
                if (this.name === "vaults") {
                    if (phase === "publication") {
                        faults++;
                        throw new DOMException(
                            "synthetic",
                            "QuotaExceededError",
                        );
                    }
                    if (phase === "uncertain") published = true;
                }
                return put.call(this, value, key);
            });
        const read = vi
            .spyOn(IDBObjectStore.prototype, "getAll")
            .mockImplementation(function (this: IDBObjectStore, ...args) {
                if (this.name === "vaults" && published) {
                    faults++;
                    throw new DOMException("synthetic", "UnknownError");
                }
                return getAll.apply(this, args);
            });
        await expect(preview.commit()).rejects.toThrow(
            "could not be confirmed",
        );
        expect(faults).toBeGreaterThan(0);
        write.mockRestore();
        read.mockRestore();
        await expect(preview.commit()).rejects.toThrow("closed");
        const id = vault.id;
        vault.close();
        const reopened = await unlockVault(password, id);
        const after = await reopened.readItems();
        if (phase !== "uncertain") expect(after).toEqual(before);
        else {
            expect(after.items).toHaveLength(5);
            expect(after.folders).toHaveLength(3);
            expect(after.folders).toContainEqual(folder);
            expect(
                after.items.every(
                    (item) =>
                        !item.folderId ||
                        after.folders.some(
                            (folder) => folder.id === item.folderId,
                        ),
                ),
            ).toBe(true);
        }
        reopened.close();
    },
    20000,
);

it("rejects stale previews and concurrent confirmations rather than overwriting or silently repeating", async () => {
    const vault = await createVault(
        password,
        `Stale import ${crypto.randomUUID()}`,
    );
    const text = JSON.stringify(supported);
    const [first, second] = await Promise.all([
        preparePersonalJsonImport(vault, text),
        preparePersonalJsonImport(vault, text),
    ]);
    const outcomes = await Promise.allSettled([
        first.commit(),
        second.commit(),
    ]);
    expect(outcomes.map((entry) => entry.status).sort()).toEqual([
        "fulfilled",
        "rejected",
    ]);
    expect((await vault.readItems()).items).toHaveLength(5);
    const preview = await preparePersonalJsonImport(vault, text);
    await vault.createFolder("Changed since preview");
    await expect(preview.commit("import-as-new")).rejects.toThrow("changed");
    preview.retire();
    first.retire();
    second.retire();
    vault.close();
});

it("a closed handle cannot submit a prepared import", async () => {
    const vault = await createVault(
        password,
        `Retired import ${crypto.randomUUID()}`,
    );
    const preview = await preparePersonalJsonImport(
        vault,
        JSON.stringify(supported),
    );
    const id = vault.id;
    vault.close();
    await expect(preview.commit()).rejects.toThrow("not submitted");
    preview.retire();
    const reopened = await unlockVault(password, id);
    expect((await reopened.readItems()).items).toEqual([]);
    reopened.close();
});
