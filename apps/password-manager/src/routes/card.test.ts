import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import type { Login, Secret, Card } from "$lib/secrets/secret";
import { prepareBackupRestore } from "$lib/vault/backup";
import { createVault, unlockVault, VaultHandle } from "$lib/vault/vault";
import Page from "./+page.svelte";

const password = "synthetic-card-master";
const date = "2026-01-02T03:04:05.000Z";
const body = "SYNTHETIC-NOTE 秘密 📝\nSecond line\n<script>plain text</script>";
const card: Card = {
    kind: "card",
    cardholderName: "山田 Synthetic",
    brand: "Unknown brand",
    number: "0000 1234 5678 0000",
    expMonth: "00",
    expYear: "1999",
    code: "007",
    id: "12345678-1234-4234-8234-123456789abc",
    name: "Synthetic private Card",
    notes: body,
    favorite: true,
    fields: [
        { name: "PIN", type: "hidden", value: "PRIVATE-FIELD" },
        { name: "Enabled", type: "boolean", value: "false" },
        { name: "Text", type: "text", value: null },
    ],
    history: [{ secret: "PRIVATE-HISTORY", changedAt: date }],
    folderId: "12345678-1234-4234-8234-123456789abb",
    importSource: { format: "personal-json-v1", id: "synthetic-card-source" },
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
const cardName = `★ ${card.name}`;
async function unlock(name: string) {
    await expect.poll(() => target.textContent).toContain(name);
    await click(name);
    fill("password", password);
    await click("Unlock");
    await expect.poll(() => button(login.name).disabled).toBe(false);
}
async function start(initial: Secret[] = [card, login]) {
    const name = `Card ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    await vault.prepareItems();
    await vault.mutateItems({
        folders: [{ id: card.folderId!, name: "Synthetic Card folder" }],
    });
    for (const record of initial)
        await vault.encryptAndStore(JSON.stringify(record), record.id);
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
        return await Promise.all(
            (await vault.loadSecrets()).map(
                async (record) =>
                    JSON.parse(await vault.decryptSecret(record)) as Secret,
            ),
        );
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

it("creates and edits Cards with Unicode and multiline notes without Login fields, then reopens and restores an exact mixed vault", async () => {
    const logs = [vi.spyOn(console, "log"), vi.spyOn(console, "info"), vi.spyOn(console, "warn"), vi.spyOn(console, "error")];
    const { id, name } = await start();
    await click(cardName);
    expect(target.querySelector("form")).toBeNull();
    expect(target.innerHTML).not.toContain("PRIVATE-");
    expect(target.innerHTML).not.toContain(card.number);
    expect(target.textContent).not.toContain(card.code);
    const copy = vi
        .spyOn(navigator.clipboard, "writeText")
        .mockResolvedValueOnce();
    await click("Copy Card number");
    await expect.poll(() => target.textContent).toContain("Copied.");
    expect(copy).toHaveBeenCalledWith(card.number);
    copy.mockRejectedValueOnce(new Error(`Denied ${card.code}`));
    await click("Copy Security code");
    await expect.poll(() => target.textContent).toContain("Copy failed.");
    expect(target.textContent).not.toContain(`Denied ${card.code}`);
    expect(target.textContent).not.toContain(card.code);
    await click("Show Security code");
    expect(target.textContent).toContain(card.code);
    await click("Show Card number");
    expect(target.textContent).toContain(card.number);
    expect(target.querySelector("script")).toBeNull();
    await click("Edit");
    expect(document.activeElement).toBe(control("name"));
    expect(target.querySelector("#secret")).toBeNull();
    expect((control("number") as HTMLInputElement).type).toBe("password");
    expect((control("code") as HTMLInputElement).type).toBe("password");
    fill("number", "abandoned");
    fill("notes", "abandoned");
    fill("field-value-0", "abandoned");
    await click("Cancel");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    expect(await persisted(id)).toEqual(expect.arrayContaining([card, login]));
    await click("Edit");
    fill("number", "0000 9999 0000 1111");
    fill("notes", `${body}\nSaved edit`);
    await click("Save");
    await saved();
    const changed = (await persisted(id)).find(
        (record) => record.id === card.id,
    )!;
    expect(changed).toEqual({
        ...card,
        number: "0000 9999 0000 1111",
        notes: `${body}\nSaved edit`,
        updatedAt: expect.any(String),
    });
    expect(changed.updatedAt).not.toBe(date);
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("New card");
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.querySelector("#username")).toBeNull();
    expect(target.querySelector("#totp")).toBeNull();
    fill("name", "  ");
    fill("notes", body);
    await click("Save");
    expect(target.textContent).toContain("Invalid Card");
    expect(await persisted(id)).toHaveLength(2);
    fill("name", "Created Card");
    fill("number", card.number!);
    fill("code", card.code!);
    fill("cardholderName", card.cardholderName!);
    fill("brand", card.brand!);
    fill("expMonth", card.expMonth!);
    fill("expYear", card.expYear!);
    await click("Save");
    await saved();
    const expected = await persisted(id);
    const created = expected.find((record) => record.name === "Created Card")!;
    expect(created).toMatchObject({
        kind: "card",
        cardholderName: "山田 Synthetic",
        brand: "Unknown brand",
        number: "0000 1234 5678 0000",
        expMonth: "00",
        expYear: "1999",
        code: "007",
        notes: body,
        history: [],
    });
    expect(created).not.toHaveProperty("secret");
    expect(expected.find((record) => record.id === login.id)).toEqual(login);
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Lock vault");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await unlock(name);
    await click("Created Card");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Show Card number");
    expect(target.textContent).toContain(card.number);
    await click("Lock vault");
    const source = await unlockVault(password, id);
    const backup = await source.exportBackup();
    const encrypted = await source.loadSecrets();
    source.close();
    for (const marker of [
        "PRIVATE-",
        card.number!,
        card.cardholderName!,
        card.brand!,
        body,
        "0000 9999 0000 1111",
    ]) {
        expect(JSON.stringify(encrypted)).not.toContain(marker);
        expect(backup).not.toContain(marker);
    }
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
    await expect(
        prepareBackupRestore("wrong-password", backup),
    ).rejects.toThrow();
    await (await prepareBackupRestore(password, backup)).restore();
    expect(await persisted(id)).toEqual(expected);
    const logged = JSON.stringify(logs.flatMap((log) => log.mock.calls));
    expect(logged).not.toContain(card.number);
    expect(logged).not.toContain("PRIVATE-");
}, 25000);

it.each([undefined, null, ""])(
    "retains all optional Card fields (%s) and common metadata on save",
    async (value) => {
        const original = {
            ...card,
            cardholderName: value,
            brand: value,
            number: value,
            expMonth: value,
            expYear: value,
            code: value,
            notes: value,
        };
        const { id } = await start([original, login]);
        await click(cardName);
        await click("Edit");
        fill("name", "Renamed Card");
        await click("Save");
        await saved();
        expect(
            (await persisted(id)).find((record) => record.id === card.id),
        ).toEqual({
            ...original,
            name: "Renamed Card",
            updatedAt: expect.any(String),
        });
        expect(
            (await persisted(id)).find((record) => record.id === login.id),
        ).toEqual(login);
    },
    15000,
);

it.each(["encryption", "readback"] as const)(
    "does not claim a Card save after %s failure and recovers the authoritative record",
    async (operation) => {
        const { id } = await start();
        await click(cardName);
        await click("Edit");
        fill("number", "PRIVATE-CHANGED");
        if (operation === "encryption") {
            vi.spyOn(crypto.subtle, "encrypt").mockRejectedValueOnce(
                new Error("Synthetic quota/write failure"),
            );
        } else {
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
        await expect.poll(() => button(cardName).disabled).toBe(false);
        await click(cardName);
        expect(target.innerHTML).not.toContain("PRIVATE-");
        expect(
            (await persisted(id)).find((record) => record.id === card.id),
        ).toEqual(
            operation === "encryption"
                ? card
                : {
                      ...card,
                      number: "PRIVATE-CHANGED",
                      updatedAt: expect.any(String),
                  },
        );
        expect(
            (await persisted(id)).find((record) => record.id === login.id),
        ).toEqual(login);
    },
    15000,
);

it.each(["draft", "copy", "read", "write", "acknowledgement"] as const)(
    "locks a Card during %s and rejects late UI results",
    async (operation) => {
        const { id, name } = await start();
        await click(cardName);
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
            await click("Show Card number");
            await click("Copy Card number");
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
            fill("number", "PRIVATE-LATE");
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
        await click(cardName);
        expect(target.innerHTML).not.toContain("PRIVATE-");
        expect(
            (await persisted(id)).find((record) => record.id === card.id),
        ).toEqual(
            operation === "acknowledgement"
                ? {
                      ...card,
                      number: "PRIVATE-LATE",
                      updatedAt: expect.any(String),
                  }
                : card,
        );
        expect(
            (await persisted(id)).find((record) => record.id === login.id),
        ).toEqual(login);
    },
    15000,
);

it("creates a name-only Card and rejects non-string fields and unknown keys without changing storage", async () => {
    const { id } = await start();
    await click("New card");
    fill("name", "Minimal Card");
    await click("Save");
    await saved();
    const expected = await persisted(id);
    expect(expected.find((item) => item.name === "Minimal Card")).toMatchObject(
        { kind: "card", number: "", code: "" },
    );
    const vault = await unlockVault(password, id);
    try {
        await vault.prepareItems();
        for (const key of [
            "cardholderName",
            "brand",
            "number",
            "expMonth",
            "expYear",
            "code",
        ]) {
            await expect(
                vault.encryptAndStore(
                    JSON.stringify({ ...card, [key]: 123 }),
                    card.id,
                ),
            ).rejects.toThrow();
        }
        await expect(
            vault.encryptAndStore(
                JSON.stringify({ ...card, secret: "not a Login" }),
                card.id,
            ),
        ).rejects.toThrow();
    } finally {
        vault.close();
    }
    expect(await persisted(id)).toEqual(expected);
}, 20000);
