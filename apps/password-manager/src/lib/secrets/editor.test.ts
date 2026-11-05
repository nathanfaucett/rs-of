import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { EncryptedSecret, VaultHandle } from "../vault/vault";
import LocalSecretEditor from "./LocalSecretEditor.svelte";
import type { Secret } from "./secret";

const original: Secret = {
    id: "12345678-1234-4234-8234-123456789abc",
    name: "Personal login",
    secret: "old-password",
    username: "old-user",
    uris: [
        { uri: "https://example.com", match: "domain" },
        { uri: "https://work.example.com/login", match: "exact" },
    ],
    fields: [
        { name: "Account", value: "123", type: "text" },
        { name: "Enabled", value: "true", type: "boolean" },
    ],
    favorite: false,
    notes: "Original notes",
    history: [],
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
};
const other: Secret = {
    ...original,
    id: "12345678-1234-4234-8234-123456789abd",
    name: "Work login",
};

let target: HTMLDivElement;
let component: ReturnType<typeof mount> | undefined;

function button(name: string) {
    const element = [...target.querySelectorAll("button")].find(
        (value) => value.textContent?.trim() === name,
    );
    if (!element) throw new Error(`Missing button: ${name}`);
    return element;
}

async function click(name: string) {
    const element = button(name);
    expect(element.disabled).toBe(false);
    element.click();
    flushSync();
    await tick();
}

function control(id: string) {
    const element = target.querySelector<
        HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement
    >(`#${id}`);
    if (!element) throw new Error(`Missing control: ${id}`);
    return element;
}

async function fill(id: string, value: string) {
    const element = control(id);
    element.value = value;
    element.dispatchEvent(
        new Event(element instanceof HTMLSelectElement ? "change" : "input", {
            bubbles: true,
        }),
    );
    flushSync();
    await tick();
}

async function setup(initial: Secret[] = []) {
    const records = new Map(
        initial.map((record) => [record.id, structuredClone(record)]),
    );
    function encrypted(record: Secret): EncryptedSecret {
        return {
            id: record.id,
            ciphertext: JSON.stringify(record),
            nonce: "test",
            createdAt: record.createdAt,
            updatedAt: record.updatedAt,
        };
    }
    const vault = {
        id: crypto.randomUUID(),
        loadSecrets: vi.fn(async () => [...records.values()].map(encrypted)),
        decryptSecret: vi.fn(
            async (record: EncryptedSecret) => record.ciphertext,
        ),
        encryptAndStore: vi.fn(async (plaintext: string, id?: string) => {
            const record: Secret = JSON.parse(plaintext);
            expect(id).toBe(record.id);
            records.set(record.id, record);
            return encrypted(record);
        }),
        deleteSecret: vi.fn(async (id: string) => {
            records.delete(id);
        }),
        close: vi.fn(),
    } satisfies Pick<
        VaultHandle,
        | "id"
        | "loadSecrets"
        | "decryptSecret"
        | "encryptAndStore"
        | "deleteSecret"
        | "close"
    >;
    target = document.createElement("div");
    document.body.append(target);
    component = mount(LocalSecretEditor, {
        target,
        props: { vault: vault as unknown as VaultHandle },
    });
    flushSync();
    await expect.poll(() => button("New secret").disabled).toBe(false);
    return { records, vault };
}

async function saved() {
    await expect.poll(() => target.textContent).toContain("Saved.");
    expect(button("Save").disabled).toBe(false);
}

afterEach(async () => {
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
});

describe("local secret editor with the real workflow and collection", () => {
    it("adds and saves two websites, match rules, username, and custom field types", async () => {
        const { records, vault } = await setup();
        await click("New secret");
        await fill("name", "New login");
        await fill("secret", "new-password");
        await fill("confirm-secret", "new-password");
        await fill("username", "alice");
        await click("Add website");
        await fill("uri-0", "https://example.com");
        await fill("match-0", "host");
        await click("Add website");
        await fill("uri-1", "https://work.example.com/login");
        await fill("match-1", "startsWith");
        for (const [index, name, type, value] of [
            [0, "Account", "text", "456"],
            [1, "PIN", "hidden", "9876"],
            [2, "Enabled", "boolean", "false"],
        ] as const) {
            await click("Add custom field");
            await fill(`field-name-${index}`, name);
            await fill(`field-type-${index}`, type);
            await fill(`field-value-${index}`, value);
        }
        expect((control("field-value-1") as HTMLInputElement).type).toBe(
            "password",
        );
        expect(control("field-value-2")).toBeInstanceOf(HTMLSelectElement);
        expect(records.size).toBe(0);
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
        await click("Save");
        await saved();

        expect(records.size).toBe(1);
        const record = [...records.values()][0];
        if (!record) throw new Error("Expected the saved login");
        expect(record).toMatchObject({
            name: "New login",
            secret: "new-password",
            username: "alice",
            uris: [
                { uri: "https://example.com", match: "host" },
                { uri: "https://work.example.com/login", match: "startsWith" },
            ],
            fields: [
                { name: "Account", type: "text", value: "456" },
                { name: "PIN", type: "hidden", value: "9876" },
                { name: "Enabled", type: "boolean", value: "false" },
            ],
        });
        expect(vault.encryptAndStore).toHaveBeenCalledExactlyOnceWith(
            JSON.stringify(record),
            record.id,
        );
        await click("Refresh");
        await expect.poll(() => target.textContent).toContain("Refreshed.");
        await click("New login");
        expect(control("username").value).toBe("alice");
        expect(control("uri-1").value).toBe(record.uris[1]?.uri);
        expect(control("match-1").value).toBe("startsWith");
        expect(control("field-value-2").value).toBe("false");
    });

    it("edits both websites and match rules and saves the same record", async () => {
        const { records, vault } = await setup([original, other]);
        await click(original.name);
        expect(control("username").value).toBe("old-user");
        await fill("username", "new-user");
        await fill("secret", "changed-password");
        await fill("confirm-secret", "changed-password");
        await fill("uri-0", "https://new.example.com");
        await fill("match-0", "regex");
        await fill("uri-1", "https://private.example.com");
        await fill("match-1", "never");
        await fill("field-name-0", "Customer");
        await fill("field-type-0", "hidden");
        await fill("field-value-0", "789");
        await fill("field-value-1", "false");
        await click("Save");
        await saved();

        expect(records.size).toBe(2);
        expect(records.get(original.id)).toMatchObject({
            id: original.id,
            createdAt: original.createdAt,
            username: "new-user",
            secret: "changed-password",
            uris: [
                { uri: "https://new.example.com", match: "regex" },
                { uri: "https://private.example.com", match: "never" },
            ],
            fields: [
                { name: "Customer", type: "hidden", value: "789" },
                { name: "Enabled", type: "boolean", value: "false" },
            ],
            history: [
                { secret: "old-password", changedAt: expect.any(String) },
            ],
        });
        expect(records.get(other.id)).toEqual(other);
        expect(vault.encryptAndStore).toHaveBeenCalledTimes(1);
        expect(control("match-0").value).toBe("regex");
        expect(control("match-1").value).toBe("never");
    });

    it("requires matching confirmation for new and changed passwords, and resets visibility", async () => {
        const { records, vault } = await setup([original, other]);
        await click("New secret");
        await fill("name", "Confirmed login");
        await fill("secret", "first-password");
        expect(control("confirm-secret").required).toBe(true);
        await click("Save");
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
        await fill("confirm-secret", "wrong-password");
        await click("Save");
        expect(target.textContent).toContain("Passwords do not match.");
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
        await click("Show password");
        expect((control("secret") as HTMLInputElement).type).toBe("text");
        await click("Hide password");
        expect((control("secret") as HTMLInputElement).type).toBe("password");
        await fill("confirm-secret", "first-password");
        await click("Save");
        await saved();
        expect(target.querySelector("#confirm-secret")).toBeNull();
        await click("Show password");
        await click(original.name);
        expect((control("secret") as HTMLInputElement).type).toBe("password");
        expect(target.querySelector("#confirm-secret")).toBeNull();
        await fill("notes", "Notes only");
        await click("Save");
        await saved();
        expect(records.get(original.id)?.notes).toBe("Notes only");
        await fill("secret", "second-password");
        await fill("confirm-secret", "wrong-password");
        await click("Save");
        expect(records.get(original.id)?.secret).toBe(original.secret);
        await fill("confirm-secret", "second-password");
        await click("Save");
        await saved();
        expect(records.get(original.id)?.secret).toBe("second-password");
        await fill("secret", "third-password");
        expect(control("confirm-secret").value).toBe("");
        await fill("confirm-secret", "third-password");
        await click("Save");
        await saved();
        expect(records.get(original.id)?.secret).toBe("third-password");
        expect(records.get(other.id)).toEqual(other);
        expect(JSON.stringify([...records.values()])).not.toContain(
            "confirmPassword",
        );
    });

    it("Cancel discards nested edits without changing the collection or persisted record", async () => {
        const { records, vault } = await setup([original]);
        await click(original.name);
        await fill("name", "Discarded login");
        await fill("username", "discarded-user");
        await fill("uri-0", "https://discarded.example.com");
        await fill("match-0", "never");
        await fill("field-name-0", "Discarded field");
        await fill("field-value-0", "discarded");
        await click("Add website");
        await click("Add custom field");
        await click("Cancel");
        expect(target.querySelector("form")).toBeNull();
        expect(records.get(original.id)).toEqual(original);
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
        expect(vault.deleteSecret).not.toHaveBeenCalled();

        await click(original.name);
        expect(control("username").value).toBe(original.username);
        expect(control("uri-0").value).toBe(original.uris[0]?.uri);
        expect(control("match-0").value).toBe("domain");
        expect(control("field-name-0").value).toBe("Account");
        expect(control("field-value-0").value).toBe("123");
        expect(target.querySelector("#uri-2")).toBeNull();
        expect(target.querySelector("#field-name-2")).toBeNull();
        await click("Cancel");
        await click("New secret");
        await fill("name", "Never saved");
        await click("Cancel");
        expect(records.size).toBe(1);
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
    });

    it("requires delete confirmation, supports cancellation, and removes only the selected record", async () => {
        const { records, vault } = await setup([original, other]);
        await click(original.name);
        await click("Delete");
        expect(target.textContent).toContain(
            `Delete ${original.name}? This cannot be undone.`,
        );
        expect(records.get(original.id)).toEqual(original);
        expect(vault.deleteSecret).not.toHaveBeenCalled();
        await click("Cancel delete");
        expect(target.textContent).not.toContain("This cannot be undone.");
        expect(target.querySelector("form")).not.toBeNull();
        expect(vault.deleteSecret).not.toHaveBeenCalled();
        await click("Delete");
        await click("Confirm delete");
        await expect.poll(() => target.textContent).toContain("Deleted.");
        expect(vault.deleteSecret).toHaveBeenCalledExactlyOnceWith(original.id);
        expect(records.has(original.id)).toBe(false);
        expect(records.get(other.id)).toEqual(other);
        expect(target.querySelector("form")).toBeNull();
        expect(target.textContent).not.toContain(original.name);
        expect(button(other.name)).toBeDefined();
        await click("Refresh");
        await expect.poll(() => target.textContent).toContain("Refreshed.");
        expect(target.textContent).not.toContain(original.name);
        expect(button(other.name)).toBeDefined();
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
    });
});
