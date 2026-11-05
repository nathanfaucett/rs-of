import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import type { Identity, Login, Secret } from "$lib/secrets/secret";
import { prepareBackupRestore } from "$lib/vault/backup";
import { createVault, unlockVault, VaultHandle } from "$lib/vault/vault";
import Page from "./+page.svelte";

const password = "synthetic-identity-master";
const date = "2026-01-02T03:04:05.000Z";
const values = {
    title: "博士",
    firstName: "Zoë",
    middleName: "李",
    lastName: "O’Neill",
    address1: "東京都千代田区\n架空 001",
    address2: "Bâtiment β",
    address3: "الطابق ٢",
    city: "東京",
    state: "Île-de-France",
    postalCode: "00100-α",
    country: "日本 / 日本国",
    company: "Société fictive",
    email: "synthetic (at) example.invalid",
    phone: "+81 (0)3 0000 ext. ０１",
    ssn: "PRIVATE-SSN-000",
    username: "synthetic-identity",
    passportNumber: "PRIVATE-PASSPORT-001",
    licenseNumber: "PRIVATE-LICENSE-002",
};
const identity: Identity = {
    ...values,
    kind: "identity",
    id: "12345678-1234-4234-8234-123456789abc",
    name: "Synthetic Identity",
    notes: "Private notes\n<script>plain text</script>",
    favorite: true,
    fields: [
        { name: "PIN", type: "hidden", value: "PRIVATE-FIELD" },
        { name: "Enabled", type: "boolean", value: "false" },
        { name: "Text", type: "text", value: null },
    ],
    history: [{ secret: "PRIVATE-HISTORY", changedAt: date }],
    folderId: "12345678-1234-4234-8234-123456789abb",
    importSource: {
        format: "personal-json-v1",
        id: "synthetic-identity-source",
    },
    createdAt: date,
    updatedAt: date,
};
const login: Login = {
    kind: "login",
    id: "12345678-1234-4234-8234-123456789abd",
    name: "Unchanged Login",
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
async function unlock(name: string) {
    await expect.poll(() => target.textContent).toContain(name);
    await click(name);
    fill("password", password);
    await click("Unlock");
    await expect.poll(() => button(login.name).disabled).toBe(false);
}
async function start(record: Identity = identity) {
    const name = `Identity ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    await vault.prepareItems();
    await vault.mutateItems({
        folders: [{ id: identity.folderId!, name: "Synthetic folder" }],
        upsert: [record, login],
    });
    await vault.saveLockSettings({ timeoutMinutes: 1 });
    const id = vault.id;
    vault.close();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
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
        return await vault.readItems();
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

it("creates all international Identity fields, cancels isolated edits, reopens, and restores the complete mixed vault in clean storage", async () => {
    const { id, name } = await start();
    await click("★ Synthetic Identity");
    expect(target.querySelector("form")).toBeNull();
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Show Passport number");
    expect(target.textContent).toContain(values.passportNumber);
    await click("Edit");
    expect(document.activeElement).toBe(control("name"));
    expect(target.querySelector("#secret")).toBeNull();
    for (const [key, value] of Object.entries(values))
        expect(control(`identity-${key}`).value).toBe(value);
    fill("identity-firstName", "abandoned");
    fill("field-value-0", "abandoned");
    fill("notes", "abandoned");
    await click("Cancel");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    expect((await persisted(id)).items).toEqual(
        expect.arrayContaining([identity, login]),
    );
    await click("Edit");
    fill("identity-firstName", "Renée 李");
    await click("Save");
    await saved();
    expect(
        (await persisted(id)).items.find((item) => item.id === identity.id),
    ).toEqual({
        ...identity,
        firstName: "Renée 李",
        updatedAt: expect.any(String),
    });
    await click("New identity");
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.querySelector("#totp")).toBeNull();
    fill("name", "  ");
    await click("Save");
    expect(target.textContent).toContain("Invalid Identity");
    expect((await persisted(id)).items).toHaveLength(2);
    fill("name", "Created Identity");
    for (const [key, value] of Object.entries(values))
        fill(`identity-${key}`, value);
    fill("notes", "Unicode 🪪\n<script>plain text</script>");
    await click("Save");
    await saved();
    const writer = await unlockVault(password, id);
    await writer.mutateItems({
        upsert: [undefined, null, ""].map((value, index) => ({
            ...identity,
            ...Object.fromEntries(
                Object.keys(values).map((key) => [key, value]),
            ),
            id: crypto.randomUUID(),
            name: `Backup sparse Identity ${index}`,
            notes: value,
        })),
    });
    writer.close();
    const expected = await persisted(id);
    expect(
        expected.items.find((item) => item.name === "Created Identity"),
    ).toMatchObject({ ...values, kind: "identity", history: [] });
    expect(expected.items.find((item) => item.id === login.id)).toEqual(login);
    expect(target.querySelector("script")).toBeNull();
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Lock vault");
    await unlock(name);
    await click("Created Identity");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Show Social security number");
    expect(target.textContent).toContain(values.ssn);
    await click("Lock vault");
    const source = await unlockVault(password, id);
    const backup = await source.exportBackup();
    const encrypted = await source.loadSecrets();
    source.close();
    expect(JSON.stringify(encrypted)).not.toContain("PRIVATE-");
    expect(backup).not.toContain("PRIVATE-");
    await unmount(component!);
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
    await expect(
        prepareBackupRestore("wrong-password", backup),
    ).rejects.toThrow();
    await (await prepareBackupRestore(password, backup)).restore();
    expect(await persisted(id)).toEqual(expected);
    const restored = await unlockVault(password, id);
    expect(await restored.loadLockSettings()).toEqual({ timeoutMinutes: 1 });
    restored.close();
}, 25000);

it.each([undefined, null, ""])(
    "preserves every unchanged optional Identity value (%s), metadata, and neighboring item",
    async (value) => {
        const sparse = {
            ...identity,
            ...Object.fromEntries(
                Object.keys(values).map((key) => [key, value]),
            ),
            notes: value,
        };
        const { id, name } = await start(sparse);
        await click("★ Synthetic Identity");
        await click("Edit");
        fill("name", "Renamed Identity");
        await click("Save");
        await saved();
        await click("Lock vault");
        await unlock(name);
        const expected = {
            ...sparse,
            name: "Renamed Identity",
            updatedAt: expect.any(String),
        };
        const snapshot = await persisted(id);
        expect(snapshot.items.find((item) => item.id === identity.id)).toEqual(
            expected,
        );
        expect(snapshot.items.find((item) => item.id === login.id)).toEqual(
            login,
        );
        await click("New identity");
        fill("name", "Sparse Identity");
        await click("Save");
        await saved();
        expect(
            (await persisted(id)).items.find(
                (item) => item.name === "Sparse Identity",
            ),
        ).toMatchObject({ kind: "identity", fields: [], history: [] });
    },
    15000,
);

it.each(["encryption", "readback"] as const)(
    "reports Identity %s failure without false success and reloads authoritative data",
    async (operation) => {
        const { id } = await start();
        await click("★ Synthetic Identity");
        await click("Edit");
        fill("identity-ssn", "PRIVATE-CHANGED");
        if (operation === "encryption")
            vi.spyOn(crypto.subtle, "encrypt").mockRejectedValueOnce(
                new Error("Synthetic write failure"),
            );
        else {
            const store = VaultHandle.prototype.encryptAndStore;
            vi.spyOn(
                VaultHandle.prototype,
                "encryptAndStore",
            ).mockImplementationOnce(async function (...args) {
                const result = await store.apply(this, args);
                vi.spyOn(crypto.subtle, "decrypt").mockRejectedValueOnce(
                    new Error("Synthetic readback failure"),
                );
                return result;
            });
        }
        await click("Save");
        await expect
            .poll(() => target.querySelector('[role="alert"]')?.textContent)
            .toContain("Refresh and verify");
        expect(target.textContent).not.toContain("Saved.");
        expect(target.innerHTML).not.toContain("PRIVATE-");
        await click("Refresh");
        await expect
            .poll(() => button("★ Synthetic Identity").disabled)
            .toBe(false);
        const snapshot = await persisted(id);
        expect(snapshot.items.find((item) => item.id === identity.id)).toEqual(
            operation === "encryption"
                ? identity
                : {
                      ...identity,
                      ssn: "PRIVATE-CHANGED",
                      updatedAt: expect.any(String),
                  },
        );
        expect(snapshot.items.find((item) => item.id === login.id)).toEqual(
            login,
        );
    },
    15000,
);

it("copies identifiers without reveal and reports clipboard failure without personal data", async () => {
    await start();
    await click("★ Synthetic Identity");
    const copy = vi
        .spyOn(navigator.clipboard, "writeText")
        .mockResolvedValueOnce();
    await click("Copy Passport number");
    expect(copy).toHaveBeenCalledWith(values.passportNumber);
    await expect.poll(() => target.textContent).toContain("Copied.");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    copy.mockRejectedValueOnce(new Error(values.ssn));
    await click("Copy Social security number");
    await expect.poll(() => target.textContent).toContain("Copy failed");
    expect(target.innerHTML).not.toContain("PRIVATE-");
}, 15000);

it("locks an expired Identity draft before Save can submit it", async () => {
    const { id, name } = await start();
    await click("★ Synthetic Identity");
    await click("Show Passport number");
    await click("Edit");
    fill("identity-ssn", "PRIVATE-EXPIRED-DRAFT");
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 60_001);
    await click("Save");
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    expect(target.querySelector("#identity-ssn")).toBeNull();
    vi.restoreAllMocks();
    await unlock(name);
    expect((await persisted(id)).items).toEqual(
        expect.arrayContaining([identity, login]),
    );
}, 15000);

it.each(["draft", "copy", "read", "write", "acknowledgement"] as const)(
    "retires Identity %s on lock and rejects late results",
    async (operation) => {
        const { id, name } = await start();
        await click("★ Synthetic Identity");
        let release!: () => void;
        const gate = new Promise<void>((resolve) => {
            release = resolve;
        });
        let started = operation === "draft";
        if (operation === "copy") {
            vi.spyOn(navigator.clipboard, "writeText").mockImplementationOnce(
                async () => {
                    started = true;
                    await gate;
                },
            );
            await click("Show Passport number");
            await click("Copy Passport number");
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
            fill("identity-ssn", "PRIVATE-LATE");
            if (operation === "acknowledgement") {
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
            } else if (operation === "write") {
                const encrypt = crypto.subtle.encrypt.bind(crypto.subtle);
                vi.spyOn(crypto.subtle, "encrypt").mockImplementationOnce(
                    async (...args) => {
                        started = true;
                        await gate;
                        return encrypt(...args);
                    },
                );
            }
            if (operation !== "draft") await click("Save");
        }
        await expect.poll(() => started).toBe(true);
        await click("Lock vault");
        await expect
            .poll(() => target.textContent)
            .toContain("Select a vault to unlock");
        expect(target.querySelector("form")).toBeNull();
        expect(target.innerHTML).not.toContain("PRIVATE-");
        release();
        await new Promise((resolve) => setTimeout(resolve, 100));
        flushSync();
        expect(target.innerHTML).not.toContain("PRIVATE-");
        for (const text of ["Saved.", "Copied.", "Refreshed."])
            expect(target.textContent).not.toContain(text);
        vi.restoreAllMocks();
        await unlock(name);
        await click("★ Synthetic Identity");
        expect(target.innerHTML).not.toContain("PRIVATE-");
        expect(
            (await persisted(id)).items.find((item) => item.id === identity.id),
        ).toEqual(
            operation === "acknowledgement"
                ? {
                      ...identity,
                      ssn: "PRIVATE-LATE",
                      updatedAt: expect.any(String),
                  }
                : identity,
        );
    },
    15000,
);
