import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import { prepareBackupRestore } from "$lib/vault/backup";
import { createVault, unlockVault } from "$lib/vault/vault";
import type { Login } from "$lib/secrets/secret";
import Page from "./+page.svelte";

const password = "synthetic-detail-master";
const date = "2026-01-02T03:04:05.000Z";
const original: Login = {
    kind: "login",
    id: "12345678-1234-4234-8234-123456789abc",
    name: "Synthetic detail Login",
    username: "alice",
    secret: "PRIVATE-CURRENT",
    totp: "PRIVATE-TOTP",
    notes: "Original notes",
    favorite: false,
    uris: [{ uri: "example.com", match: null }],
    fields: [{ name: "PIN", type: "hidden", value: "PRIVATE-FIELD" }],
    history: [{ secret: "PRIVATE-HISTORY", changedAt: date }],
    createdAt: date,
    updatedAt: date,
};
let target: HTMLDivElement;
let component: ReturnType<typeof mount> | undefined;
function button(name: string) {
    const found = [...target.querySelectorAll("button")].find(
        (element) =>
            (
                element.querySelector(".vault-name")?.textContent ??
                element.textContent
            )?.trim() === name,
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
function control(id: string) {
    const found = target.querySelector<HTMLInputElement | HTMLTextAreaElement>(
        `#${id}`,
    );
    if (!found) throw new Error(`Missing control: ${id}`);
    return found;
}
function fill(id: string, value: string) {
    const found = control(id);
    found.value = value;
    found.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
}
async function start() {
    const name = `Detail ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    await vault.prepareItems();
    await vault.saveLockSettings({ timeoutMinutes: 1 });
    await vault.encryptAndStore(JSON.stringify(original), original.id);
    const id = vault.id;
    vault.close();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await expect.poll(() => target.textContent).toContain(name);
    await click(name);
    fill("password", password);
    await click("Unlock");
    await expect.poll(() => button(original.name).disabled).toBe(false);
    return { name, id };
}
async function saved() {
    await expect.poll(() => target.textContent).toContain("Saved.");
    expect(target.querySelector("form")).toBeNull();
}
async function persisted(id: string) {
    const vault = await unlockVault(password, id);
    try {
        return JSON.parse(
            await vault.decryptSecret((await vault.loadSecrets())[0]),
        ) as Login;
    } finally {
        vault.close();
    }
}
afterEach(async () => {
    vi.restoreAllMocks();
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
});

it("preserves read-only data, isolated cancel, dated history and notes-only edits through reopen and clean backup restore", async () => {
    const { id, name } = await start();
    await click(original.name);
    expect(target.querySelector("form")).toBeNull();
    for (const secret of [
        original.secret,
        original.totp,
        "PRIVATE-FIELD",
        "PRIVATE-HISTORY",
    ])
        expect(target.innerHTML).not.toContain(secret);
    expect(await persisted(id)).toEqual(original);
    expect(target.querySelector("time")?.dateTime).toBe(date);
    await click("Show History password 1");
    expect(target.textContent).toContain("PRIVATE-HISTORY");
    await click("Edit");
    expect(document.activeElement).toBe(control("name"));
    fill("uri-0", "discarded.example");
    fill("field-value-0", "discarded");
    fill("secret", "abandoned-password");
    await click("Cancel");
    expect(target.querySelector("form")).toBeNull();
    expect(target.innerHTML).not.toContain("PRIVATE-HISTORY");
    expect(await persisted(id)).toEqual(original);
    await click("Edit");
    fill("notes", "Saved notes only");
    await click("Save");
    await saved();
    expect((await persisted(id)).history).toEqual(original.history);
    await click("Edit");
    fill("secret", "PRIVATE-REPLACEMENT");
    fill("confirm-secret", "PRIVATE-REPLACEMENT");
    await click("Save");
    await saved();
    expect(target.textContent).toContain("Password history (2)");
    const changed = await persisted(id);
    expect(changed.createdAt).toBe(date);
    expect(changed.history).toEqual([
        original.history[0],
        { secret: original.secret, changedAt: changed.updatedAt },
    ]);
    const write = vi
        .spyOn(navigator.clipboard, "writeText")
        .mockResolvedValue();
    await click("Copy History password 2");
    await expect.poll(() => target.textContent).toContain("Copied.");
    expect(write).toHaveBeenCalledWith(original.secret);
    expect(target.innerHTML).not.toContain("PRIVATE-CURRENT");
    await click("Lock vault");
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click(name);
    fill("password", password);
    await click("Unlock");
    await expect.poll(() => button(original.name).disabled).toBe(false);
    await click(original.name);
    expect(target.innerHTML).not.toContain("PRIVATE-");
    expect(target.textContent).toContain("Saved notes only");
    expect(target.textContent).toContain("Password history (2)");
    await click("Lock vault");
    const source = await unlockVault(password, id);
    const backup = await source.exportBackup();
    source.close();
    expect(backup).not.toContain("PRIVATE-");
    if (component) await unmount(component);
    component = undefined;
    target.remove();
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
    expect(await persisted(id)).toEqual(changed);
}, 20000);

it("shows a failed save without adding history or changing the stored Login", async () => {
    const { id } = await start();
    await click(original.name);
    await click("Edit");
    fill("secret", "never-saved");
    fill("confirm-secret", "never-saved");
    vi.spyOn(crypto.subtle, "encrypt").mockRejectedValueOnce(
        new Error("Synthetic encryption failure"),
    );
    await click("Save");
    await expect
        .poll(() => target.querySelector('[role="alert"]')?.textContent)
        .toContain("Refresh and verify");
    expect(target.textContent).not.toContain("Saved.");
    expect(await persisted(id)).toEqual(original);
    await click("Refresh");
    await expect.poll(() => button(original.name).disabled).toBe(false);
    await click(original.name);
    expect(target.textContent).toContain("Password history (1)");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Lock vault");
}, 15000);

it.each((["copy", "read", "write", "acknowledgement"] as const).flatMap((operation) =>
    (["manual", "automatic"] as const).map((mode) => ({ operation, mode })),
))(
    "$mode lock during a pending $operation does not restore secrets or feedback from its late result",
    async ({ operation, mode }) => {
        const { id, name } = await start();
        await click(original.name);
        let release!: () => void;
        const gate = new Promise<void>((resolve) => {
            release = resolve;
        });
        let started = false;
        if (operation === "copy") {
            vi.spyOn(navigator.clipboard, "writeText").mockImplementationOnce(
                async () => {
                    started = true;
                    await gate;
                },
            );
            await click("Show Password");
            await click("Copy Password");
        } else if (operation === "read") {
            const decrypt = crypto.subtle.decrypt.bind(crypto.subtle);
            vi.spyOn(crypto.subtle, "decrypt").mockImplementationOnce(
                async (...args) => {
                    started = true;
                    await gate;
                    return decrypt(...args);
                },
            );
            await click("Refresh");
        } else {
            await click("Edit");
            fill("notes", "Late write");
            if (operation === "acknowledgement") {
                const decrypt = crypto.subtle.decrypt.bind(crypto.subtle);
                vi.spyOn(crypto.subtle, "decrypt").mockImplementationOnce(
                    async (...args) => {
                        started = true;
                        await gate;
                        return decrypt(...args);
                    },
                );
            } else {
                const encrypt = crypto.subtle.encrypt.bind(crypto.subtle);
                vi.spyOn(crypto.subtle, "encrypt").mockImplementationOnce(
                    async (...args) => {
                        started = true;
                        await gate;
                        return encrypt(...args);
                    },
                );
            }
            await click("Save");
        }
        await expect.poll(() => started).toBe(true);
        if (mode === "manual") await click("Lock vault");
        else {
            vi.spyOn(Date, "now").mockReturnValue(Date.now() + 60_001);
            window.dispatchEvent(new Event("focus"));
            flushSync();
        }
        await expect
            .poll(() => target.textContent)
            .toContain("Select a vault to unlock");
        expect(target.querySelector("form")).toBeNull();
        expect(target.innerHTML).not.toContain("PRIVATE-");
        release();
        await new Promise((resolve) => setTimeout(resolve, 100));
        flushSync();
        expect(target.innerHTML).not.toContain("PRIVATE-");
        expect(target.textContent).not.toContain("Copied.");
        expect(target.textContent).not.toContain("Saved.");
        expect(target.textContent).not.toContain("Refreshed.");
        vi.restoreAllMocks();
        await click(name);
        fill("password", password);
        await click("Unlock");
        await expect.poll(() => button(original.name).disabled).toBe(false);
        await click(original.name);
        expect(target.innerHTML).not.toContain("PRIVATE-");
        const record = await persisted(id);
        if (operation === "acknowledgement") {
            expect(target.textContent).toContain("Late write");
            expect(record).toEqual({
                ...original,
                notes: "Late write",
                updatedAt: expect.any(String),
            });
        } else {
            expect(record).toEqual(original);
        }
    },
    15000,
);
