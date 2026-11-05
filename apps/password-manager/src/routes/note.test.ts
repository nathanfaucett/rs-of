import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import { prepareBackupRestore } from "$lib/vault/backup";
import { createVault, unlockVault } from "$lib/vault/vault";
import type { Login, Secret, SecureNote } from "$lib/secrets/secret";
import Page from "./+page.svelte";

const password = "synthetic-note-master";
const date = "2026-01-02T03:04:05.000Z";
const body = "PRIVATE-NOTE 秘密 📝\nSecond line\n<script>plain text</script>";
const note: SecureNote = {
    kind: "secureNote",
    type: 0,
    id: "12345678-1234-4234-8234-123456789abc",
    name: "Synthetic private Note",
    notes: body,
    favorite: true,
    fields: [
        { name: "PIN", type: "hidden", value: "PRIVATE-FIELD" },
        { name: "Enabled", type: "boolean", value: "false" },
        { name: "Text", type: "text", value: null },
    ],
    history: [{ secret: "PRIVATE-HISTORY", changedAt: date }],
    folderId: "12345678-1234-4234-8234-123456789abb",
    importSource: { format: "personal-json-v1", id: "synthetic-note-source" },
    createdAt: date,
    updatedAt: date,
};
const login: Login = {
    kind: "login",
    id: "12345678-1234-4234-8234-123456789abd",
    name: "Synthetic unchanged Login",
    secret: "PRIVATE-LOGIN",
    username: null,
    totp: "",
    notes: "Login notes",
    favorite: false,
    fields: [],
    history: [],
    uris: [{ uri: "example.com", match: null }],
    createdAt: date,
    updatedAt: date,
};
let target: HTMLDivElement;
let component: ReturnType<typeof mount> | undefined;
function button(name: string) {
    const found = [...target.querySelectorAll("button")].find((element) =>
        (element.querySelector(".vault-name")?.textContent ?? element.textContent)?.trim() === name,
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
    const found = target.querySelector<HTMLInputElement | HTMLTextAreaElement>(`#${id}`);
    if (!found) throw new Error(`Missing control: ${id}`);
    return found;
}
function fill(id: string, value: string) {
    const found = control(id);
    found.value = value;
    found.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
}
const noteName = `★ ${note.name}`;
async function unlock(name: string) {
    await expect.poll(() => target.textContent).toContain(name);
    await click(name);
    fill("password", password);
    await click("Unlock");
    await expect.poll(() => button(login.name).disabled).toBe(false);
}
async function start(initial: Secret[] = [note, login]) {
    const name = `Note ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    await vault.prepareItems();
    for (const record of initial) await vault.encryptAndStore(JSON.stringify(record), record.id);
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
async function saved() {
    await expect.poll(() => target.textContent).toContain("Saved.");
    expect(target.querySelector("form")).toBeNull();
}
async function persisted(id: string) {
    const vault = await unlockVault(password, id);
    try {
        return await Promise.all((await vault.loadSecrets()).map(async (record) =>
            JSON.parse(await vault.decryptSecret(record)) as Secret,
        ));
    } finally { vault.close(); }
}
afterEach(async () => {
    vi.restoreAllMocks();
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
});

it("creates and edits private Unicode multiline Notes without Login fields, then reopens and restores an exact mixed vault", async () => {
    const { id, name } = await start();
    await click(noteName);
    expect(target.querySelector("form")).toBeNull();
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Show Notes");
    expect(target.textContent).toContain(body);
    expect(target.querySelector("script")).toBeNull();
    await click("Edit");
    expect(document.activeElement).toBe(control("name"));
    expect(target.querySelector("#secret")).toBeNull();
    fill("notes", "abandoned");
    fill("field-value-0", "abandoned");
    await click("Cancel");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    expect(await persisted(id)).toEqual(expect.arrayContaining([note, login]));
    await click("Edit");
    fill("notes", `${body}\nSaved edit`);
    await click("Save");
    await saved();
    const changed = (await persisted(id)).find((record) => record.id === note.id)!;
    expect(changed).toEqual({ ...note, notes: `${body}\nSaved edit`, updatedAt: expect.any(String) });
    expect(changed.updatedAt).not.toBe(date);
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("New secure note");
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.querySelector("#username")).toBeNull();
    expect(target.querySelector("#totp")).toBeNull();
    fill("name", "  ");
    fill("notes", body);
    await click("Save");
    expect(target.textContent).toContain("Invalid Secure Note");
    expect(await persisted(id)).toHaveLength(2);
    fill("name", "Created Note");
    await click("Save");
    await saved();
    const expected = await persisted(id);
    const created = expected.find((record) => record.name === "Created Note")!;
    expect(created).toMatchObject({ kind: "secureNote", type: 0, notes: body, history: [] });
    expect(created).not.toHaveProperty("secret");
    expect(expected.find((record) => record.id === login.id)).toEqual(login);
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Lock vault");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await unlock(name);
    await click("Created Note");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Show Notes");
    expect(target.textContent).toContain(body);
    await click("Lock vault");
    const source = await unlockVault(password, id);
    const backup = await source.exportBackup();
    const encrypted = await source.loadSecrets();
    source.close();
    expect(JSON.stringify(encrypted)).not.toContain("PRIVATE-");
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
    await expect(prepareBackupRestore("wrong-password", backup)).rejects.toThrow();
    await (await prepareBackupRestore(password, backup)).restore();
    expect(await persisted(id)).toEqual(expected);
}, 25000);

it.each([{ notes: undefined }, { notes: null }, { notes: "" }])("retains an unchanged optional Note body ($notes) and common metadata on save", async ({ notes }) => {
    const original = { ...note, notes };
    const { id } = await start([original, login]);
    await click(noteName);
    await click("Edit");
    fill("name", "Renamed Note");
    await click("Save");
    await saved();
    expect((await persisted(id)).find((record) => record.id === note.id)).toEqual({
        ...original, name: "Renamed Note", updatedAt: expect.any(String),
    });
    expect((await persisted(id)).find((record) => record.id === login.id)).toEqual(login);
}, 15000);

it.each(["encryption", "readback"] as const)("does not claim a Note save after %s failure and recovers the authoritative record", async (operation) => {
    const { id } = await start();
    await click(noteName);
    await click("Edit");
    fill("notes", "PRIVATE-CHANGED");
    if (operation === "encryption") {
        vi.spyOn(crypto.subtle, "encrypt").mockRejectedValueOnce(new Error("Synthetic quota/write failure"));
    } else {
        vi.spyOn(crypto.subtle, "decrypt").mockRejectedValueOnce(new Error("Synthetic readback failure"));
    }
    await click("Save");
    await expect.poll(() => target.querySelector('[role="alert"]')?.textContent).toContain("Refresh and verify");
    expect(target.textContent).not.toContain("Saved.");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Refresh");
    await expect.poll(() => button(noteName).disabled).toBe(false);
    await click(noteName);
    expect(target.innerHTML).not.toContain("PRIVATE-");
    expect((await persisted(id)).find((record) => record.id === note.id)).toEqual(operation === "encryption"
        ? note : { ...note, notes: "PRIVATE-CHANGED", updatedAt: expect.any(String) });
    expect((await persisted(id)).find((record) => record.id === login.id)).toEqual(login);
}, 15000);

it.each(["draft", "copy", "read", "write", "acknowledgement"] as const)("locks a Note during %s and rejects late UI results", async (operation) => {
    const { id, name } = await start();
    await click(noteName);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    let started = operation === "draft";
    if (operation === "copy") {
        vi.spyOn(navigator.clipboard, "writeText").mockImplementationOnce(async () => { started = true; await gate; });
        await click("Show Notes");
        await click("Copy Notes");
    } else if (operation === "read") {
        const decrypt = crypto.subtle.decrypt.bind(crypto.subtle);
        vi.spyOn(crypto.subtle, "decrypt").mockImplementationOnce(async (...args) => { started = true; await gate; return decrypt(...args); });
        await click("Refresh");
    } else {
        await click("Edit");
        fill("notes", "PRIVATE-LATE");
        if (operation === "acknowledgement") {
            const decrypt = crypto.subtle.decrypt.bind(crypto.subtle);
            vi.spyOn(crypto.subtle, "decrypt").mockImplementationOnce(async (...args) => { started = true; await gate; return decrypt(...args); });
        } else if (operation === "write") {
            const encrypt = crypto.subtle.encrypt.bind(crypto.subtle);
            vi.spyOn(crypto.subtle, "encrypt").mockImplementationOnce(async (...args) => { started = true; await gate; return encrypt(...args); });
        }
        if (operation !== "draft") await click("Save");
    }
    await expect.poll(() => started).toBe(true);
    await click("Lock vault");
    await expect.poll(() => target.textContent).toContain("Select a vault to unlock");
    expect(target.querySelector("form")).toBeNull();
    expect(target.innerHTML).not.toContain("PRIVATE-");
    release();
    await new Promise((resolve) => setTimeout(resolve, 100));
    flushSync();
    expect(target.innerHTML).not.toContain("PRIVATE-");
    for (const text of ["Saved.", "Copied.", "Refreshed."]) expect(target.textContent).not.toContain(text);
    vi.restoreAllMocks();
    await unlock(name);
    await click(noteName);
    expect(target.innerHTML).not.toContain("PRIVATE-");
    expect((await persisted(id)).find((record) => record.id === note.id)).toEqual(operation === "acknowledgement"
        ? { ...note, notes: "PRIVATE-LATE", updatedAt: expect.any(String) } : note);
    expect((await persisted(id)).find((record) => record.id === login.id)).toEqual(login);
}, 15000);
