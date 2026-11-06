import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import type { Secret } from "$lib/secrets/secret";
import { prepareBackupRestore } from "$lib/vault/backup";
import { createVault, unlockVault, VaultHandle } from "$lib/vault/vault";
import Page from "./+page.svelte";

const password = "synthetic-trash-master";
const date = "2026-01-02T03:04:05.000Z";
const common = {
    id: "12345678-1234-4234-8234-123456789abc",
    name: "PRIVATE-TRASH-NAME 日本語",
    notes: "PRIVATE-TRASH-NOTES\nsecond line",
    favorite: true,
    fields: [
        {
            name: "PRIVATE-FIELD",
            type: "hidden" as const,
            value: "PRIVATE-VALUE",
        },
        { name: "Boolean", type: "boolean" as const, value: "false" },
        { name: "Unset", type: "text" as const, value: null },
    ],
    history: [{ secret: "PRIVATE-HISTORY", changedAt: date }],
    importSource: { format: "personal-json-v1" as const, id: "source-13" },
    folderId: null,
    createdAt: date,
    updatedAt: date,
};
const originals: Secret[] = [
    {
        ...common,
        kind: "login",
        username: "PRIVATE-USER",
        secret: "PRIVATE-PASSWORD",
        totp: "PRIVATE-TOTP",
        uris: [
            { uri: "example.com", match: null },
            { uri: "androidapp://test", match: "never" },
        ],
    },
    { ...common, kind: "secureNote", type: 0 },
    {
        ...common,
        kind: "card",
        cardholderName: "PRIVATE-CARDHOLDER",
        brand: "unrecognized",
        number: "00001234",
        expMonth: "01",
        expYear: "2000",
        code: "001",
    },
    {
        ...common,
        kind: "identity",
        title: "Mx",
        firstName: "PRIVATE-FIRST",
        middleName: "",
        lastName: null,
        address1: "PRIVATE-ADDRESS",
        address2: "Unit 1",
        address3: "Line 3",
        city: "City",
        state: "State",
        postalCode: "00123",
        country: "Country",
        company: "Company",
        email: "test@example.com",
        phone: "001234",
        ssn: "00001",
        username: "PRIVATE-USER",
        passportNumber: "00002",
        licenseNumber: "00003",
    },
    {
        ...common,
        kind: "sshKey",
        privateKey: "PRIVATE-KEY\nsynthetic only\n",
        publicKey: "PUBLIC-KEY",
        keyFingerprint: "fingerprint",
    },
];
const keeper: Secret = {
    ...originals[0],
    id: "12345678-1234-4234-8234-123456789abd",
    name: "Keep this item",
    favorite: false,
};
let target: HTMLDivElement;
let component: ReturnType<typeof mount> | undefined;
function button(name: string) {
    const found = [...target.querySelectorAll("button")].find(
        (element) =>
            (
                element.querySelector(".vault-name")?.textContent ??
                element.textContent
            )
                ?.trim()
                .replace(/^★\s*/, "") === name,
    );
    if (!found) throw new Error(`Missing button: ${name}`);
    return found;
}
async function click(name: string) {
    expect(button(name).disabled).toBe(false);
    button(name).click();
    flushSync();
    await tick();
}
async function unlock(name: string) {
    await expect.poll(() => button(name).disabled).toBe(false);
    await click(name);
    const input = target.querySelector<HTMLInputElement>("#password");
    if (!input) throw new Error("Missing password input");
    input.value = password;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    await click("Unlock");
    await expect.poll(() => button("Trash").disabled).toBe(false);
}
async function start(original: Secret = originals[0]) {
    const name = `Trash ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    await vault.prepareItems();
    if (original.folderId)
        await vault.mutateItems({
            folders: [{ id: original.folderId, name: "Retained Trash folder" }],
        });
    for (const value of [original, keeper])
        await vault.encryptAndStore(JSON.stringify(value), value.id);
    const id = vault.id;
    vault.close();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await expect.poll(() => target.textContent).toContain(name);
    await unlock(name);
    return { name, id };
}
async function persisted(id: string) {
    const vault = await unlockVault(password, id);
    try {
        await vault.prepareItems();
        const encrypted = await vault.loadSecrets();
        expect(JSON.stringify(encrypted)).not.toContain("PRIVATE-");
        return await Promise.all(
            encrypted.map(
                async (record) =>
                    JSON.parse(await vault.decryptSecret(record)) as Secret,
            ),
        );
    } finally {
        vault.close();
    }
}
async function trashItem() {
    await click("Delete");
    await click("Confirm delete");
    await expect.poll(() => target.textContent).toContain("Moved to Trash.");
}
async function purgeItem() {
    await click("Delete permanently");
    await click("Confirm permanent delete");
}
async function stop() {
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
}
async function clearStorage() {
    for (const { name } of await indexedDB.databases()) {
        if (!name) continue;
        await new Promise<void>((resolve, reject) => {
            const request = indexedDB.deleteDatabase(name);
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
            request.onblocked = () => reject(new Error("Open test database"));
        });
    }
}
afterEach(async () => {
    vi.restoreAllMocks();
    await stop();
});

it.each(["trash", "restore", "purge", "save"] as const)(
    "rejects stale %s after another session restores or edits the record",
    async (operation) => {
        const { id } = await start();
        if (operation === "restore" || operation === "purge") {
            await trashItem();
            await click("Trash");
        } else if (operation === "save") {
            await click(originals[0].name);
            await click("Edit");
        }
        const other = await unlockVault(password, id);
        const current = (await other.readItems()).items.find(
            (item) => item.id === originals[0].id,
        )!;
        const updated = {
            ...current,
            name: "Updated in session B",
            updatedAt: "2026-11-05T12:00:00.000Z",
        };
        delete updated.deletedAt;
        await other.mutateItems({ upsert: [updated] });
        other.close();
        if (operation === "purge") await purgeItem();
        else if (operation === "restore") await click("Restore");
        else if (operation === "save") await click("Save");
        else {
            await click("Delete");
            await click("Confirm delete");
        }
        await expect
            .poll(() => target.querySelector('[role="alert"]')?.textContent)
            .toContain("changed");
        expect(target.querySelector("form")).toBeNull();
        expect(await persisted(id)).toEqual(
            expect.arrayContaining([updated, keeper]),
        );
        await click("Refresh");
        await expect.poll(() => target.textContent).toContain("Refreshed.");
        await click("All items");
        expect(target.textContent).toContain(updated.name);
    },
    20000,
);

it.each(originals)(
    "keeps $kind encrypted in Trash across reopen and clean backup restore, restores all fields, and deletes only the confirmed item",
    async (original) => {
        const { id, name } = await start(original);
        await click(original.name);
        await click("Delete");
        await click("Cancel delete");
        expect(await persisted(id)).toEqual([original, keeper]);
        await trashItem();
        expect(target.textContent).not.toContain(original.name);
        const deleted = (await persisted(id))[0];
        expect(deleted).toEqual({ ...original, deletedAt: expect.any(String) });
        expect(Number.isNaN(Date.parse(deleted.deletedAt ?? ""))).toBe(false);
        await click("Trash");
        expect(target.textContent).toContain(original.name);
        expect(target.innerHTML).not.toContain("PRIVATE-HISTORY");
        expect(target.innerHTML).not.toContain("PRIVATE-TRASH-NOTES");
        await click("Delete permanently");
        expect(document.activeElement).toBe(button("Cancel delete"));
        const dialog = target.querySelector("dialog");
        dialog?.dispatchEvent(new Event("cancel", { cancelable: true }));
        flushSync();
        await tick();
        expect(await persisted(id)).toEqual([deleted, keeper]);
        await click("Lock vault");
        expect(target.innerHTML).not.toContain("PRIVATE-");
        await unlock(name);
        expect(target.textContent).not.toContain(original.name);
        await click("Trash");
        expect(target.textContent).toContain(original.name);
        await click("Lock vault");
        const source = await unlockVault(password, id);
        const backup = await source.exportBackup();
        source.close();
        expect(backup).not.toContain("PRIVATE-");
        await stop();
        await clearStorage();
        await (await prepareBackupRestore(password, backup)).restore();
        expect(await persisted(id)).toEqual([deleted, keeper]);
        target = document.createElement("div");
        document.body.append(target);
        component = mount(Page, { target });
        flushSync();
        await expect.poll(() => target.textContent).toContain(name);
        await unlock(name);
        await click("Trash");
        await click("Restore");
        await expect.poll(() => target.textContent).toContain("Restored.");
        expect(target.textContent).toContain("Trash is empty.");
        expect(await persisted(id)).toEqual([original, keeper]);
        await click("All items");
        expect(button(original.name)).toBeDefined();
        await trashItem();
        await click("Trash");
        await purgeItem();
        await expect
            .poll(() => target.textContent)
            .toContain("Permanently deleted.");
        expect(await persisted(id)).toEqual([keeper]);
        await click("Lock vault");
        await unlock(name);
        await click("Trash");
        expect(target.textContent).toContain("Trash is empty.");
        expect(await persisted(id)).toEqual([keeper]);
    },
    30000,
);

it.each(originals)(
    "restores $kind with an unresolved folder as unfiled without changing its other data",
    async (value) => {
        const original = {
            ...value,
            folderId: "12345678-1234-4234-8234-123456789abe",
        };
        const { id, name } = await start({ ...original, deletedAt: date });
        const writer = await unlockVault(password, id);
        await writer.deleteFolder(original.folderId);
        const unfiled = (await writer.readItems()).items.find(
            (item) => item.id === original.id,
        )!;
        writer.close();
        await click("Refresh");
        await expect.poll(() => target.textContent).toContain("Refreshed.");
        await click("Trash");
        await click("Restore");
        await expect.poll(() => target.textContent).toContain("Restored.");
        const restored = {
            ...original,
            folderId: null,
            updatedAt: unfiled.updatedAt,
        };
        expect(await persisted(id)).toEqual([restored, keeper]);
        await click("Lock vault");
        await unlock(name);
        expect(button(original.name)).toBeDefined();
        expect(await persisted(id)).toEqual([restored, keeper]);
    },
    15000,
);

it.each(originals)(
    "restores $kind into its retained catalog folder without changing other fields",
    async (value) => {
        const original = { ...value, folderId: crypto.randomUUID() };
        const { id } = await start({ ...original, deletedAt: date });
        await click("Trash");
        await click("Restore");
        await expect.poll(() => target.textContent).toContain("Restored.");
        expect(await persisted(id)).toEqual([original, keeper]);
    },
    15000,
);

it.each(["trash", "restore", "purge"] as const)(
    "reports failed %s and reloads the authoritative state before retry",
    async (operation) => {
        const { id } = await start();
        if (operation !== "trash") {
            await trashItem();
            await click("Trash");
        }
        const before = await persisted(id);

        if (operation === "purge") {
            // Reads after the actual deletion fail: the outcome is uncertain, not success.
            const remove = VaultHandle.prototype.deleteSecret;
            vi.spyOn(
                VaultHandle.prototype,
                "deleteSecret",
            ).mockImplementationOnce(async function (id) {
                await remove.call(this, id);
                vi.spyOn(crypto.subtle, "decrypt").mockRejectedValueOnce(
                    new Error("Synthetic read failure"),
                );
            });
            await purgeItem();
        } else {
            vi.spyOn(crypto.subtle, "encrypt").mockRejectedValueOnce(
                new Error("Synthetic write failure"),
            );
            if (operation === "trash") {
                await click("Delete");
                await click("Confirm delete");
            } else await click("Restore");
        }
        await expect
            .poll(() => target.querySelector('[role="alert"]')?.textContent)
            .toContain("Refresh and verify");
        expect(target.textContent).not.toContain(
            operation === "purge"
                ? "Permanently deleted."
                : operation === "restore"
                  ? "Restored."
                  : "Moved to Trash.",
        );
        vi.restoreAllMocks();
        expect(await persisted(id)).toEqual(
            operation === "purge" ? [keeper] : before,
        );
        await click("Refresh");
        await expect.poll(() => target.textContent).toContain("Refreshed.");
        if (operation === "trash") await trashItem();
        else if (operation === "restore") {
            await click("Restore");
            await expect.poll(() => target.textContent).toContain("Restored.");
        } else expect(target.textContent).toContain("Trash is empty.");
    },
    15000,
);

it.each(["trash", "restore", "purge"] as const)(
    "rejects a quota failure during %s without false success or data loss",
    async (operation) => {
        const { id } = await start();
        if (operation !== "trash") {
            await trashItem();
            await click("Trash");
        }
        const before = await persisted(id);
        const put = IDBObjectStore.prototype.put;
        let injected = false;
        vi.spyOn(IDBObjectStore.prototype, "put").mockImplementation(function (
            this: IDBObjectStore,
            value,
            key,
        ) {
            if (this.name === "records") {
                injected = true;
                throw new DOMException(
                    "Synthetic storage failure",
                    "QuotaExceededError",
                );
            }
            return put.call(this, value, key);
        });
        if (operation === "trash") {
            await click("Delete");
            await click("Confirm delete");
        } else if (operation === "restore") await click("Restore");
        else await purgeItem();
        await expect
            .poll(() => target.querySelector('[role="alert"]')?.textContent)
            .toContain("Refresh and verify");
        expect(injected).toBe(true);
        expect(target.textContent).not.toContain(
            operation === "purge"
                ? "Permanently deleted."
                : operation === "restore"
                  ? "Restored."
                  : "Moved to Trash.",
        );
        vi.restoreAllMocks();
        expect(await persisted(id)).toEqual(before);
        await click("Refresh");
        await expect.poll(() => target.textContent).toContain("Refreshed.");
        if (operation === "trash") await trashItem();
        else if (operation === "restore") {
            await click("Restore");
            await expect.poll(() => target.textContent).toContain("Restored.");
        } else {
            await purgeItem();
            await expect
                .poll(() => target.textContent)
                .toContain("Permanently deleted.");
        }
    },
    15000,
);

it("does not purge an old deletion date on load, refresh, or reopen", async () => {
    const old = { ...originals[0], deletedAt: "2000-01-01T00:00:00.000Z" };
    const { id, name } = await start(old);
    expect(target.textContent).not.toContain(old.name);
    await click("Trash");
    expect(target.textContent).toContain(old.name);
    await click("Refresh");
    await expect.poll(() => target.textContent).toContain("Refreshed.");
    await click("Lock vault");
    await unlock(name);
    await click("Trash");
    expect(target.textContent).toContain(old.name);
    expect(await persisted(id)).toEqual([old, keeper]);
}, 15000);

it.each(["trash", "restore", "purge"] as const)(
    "locks during pending %s acknowledgement without restoring decrypted state or success feedback",
    async (operation) => {
        const { id, name } = await start();
        if (operation !== "trash") {
            await trashItem();
            await click("Trash");
        }
        let release!: () => void;
        const gate = new Promise<void>((resolve) => {
            release = resolve;
        });
        let started = false;
        if (operation === "purge") {
            const remove = VaultHandle.prototype.deleteSecret;
            vi.spyOn(
                VaultHandle.prototype,
                "deleteSecret",
            ).mockImplementationOnce(async function (id) {
                await remove.call(this, id);
                started = true;
                await gate;
            });
        } else {
            const store = VaultHandle.prototype.encryptAndStore;
            vi.spyOn(
                VaultHandle.prototype,
                "encryptAndStore",
            ).mockImplementationOnce(async function (...args) {
                const result = await store.apply(this, args);
                started = true;
                await gate;
                return result;
            });
        }
        if (operation === "trash") {
            await click("Delete");
            await click("Confirm delete");
        } else if (operation === "restore") await click("Restore");
        else await purgeItem();
        await expect.poll(() => started).toBe(true);
        await click("Lock vault");
        expect(target.innerHTML).not.toContain("PRIVATE-");
        release();
        await new Promise((resolve) => setTimeout(resolve, 100));
        flushSync();
        expect(target.innerHTML).not.toContain("PRIVATE-");
        for (const notice of [
            "Moved to Trash.",
            "Restored.",
            "Permanently deleted.",
        ])
            expect(target.textContent).not.toContain(notice);
        vi.restoreAllMocks();
        const records = await persisted(id);
        if (operation === "purge") expect(records).toEqual([keeper]);
        else
            expect(records[0]).toEqual(
                operation === "trash"
                    ? { ...originals[0], deletedAt: expect.any(String) }
                    : originals[0],
            );
        await unlock(name);
        await click("Trash");
        expect(target.textContent).toContain(
            operation === "trash" ? common.name : "Trash is empty.",
        );
    },
    15000,
);
