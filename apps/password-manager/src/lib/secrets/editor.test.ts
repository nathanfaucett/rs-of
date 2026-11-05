import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { EncryptedSecret, VaultHandle } from "../vault/vault";
import LocalSecretEditor from "./LocalSecretEditor.svelte";
import type { Login as Secret } from "./secret";

const original: Secret = {
    kind: "login",
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
        prepareItems: vi.fn(async () => {}),
        readItems: vi.fn(async () => {
            const items = [];
            for (const record of await vault.loadSecrets())
                items.push(JSON.parse(await vault.decryptSecret(record)));
            return { items, folders: [] };
        }),
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
        | "prepareItems"
        | "readItems"
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
    expect(target.querySelector("form")).toBeNull();
}

async function edit(name: string) {
    await click(name);
    await click("Edit");
}

afterEach(async () => {
    vi.restoreAllMocks();
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
});

describe("local secret editor with the real workflow and collection", () => {
    it("creates a private multiline Secure Note without Login controls and cancels isolated edits", async () => {
        const { records } = await setup([original]);
        await click("New secure note");
        expect(target.querySelector("#secret")).toBeNull();
        expect(target.querySelector("#username")).toBeNull();
        expect(target.querySelector("#totp")).toBeNull();
        await click("Password generator");
        await click("Generate");
        expect(target.textContent).not.toContain("Use in Login draft");
        await click("Password generator");
        await fill("name", "Private note");
        await fill("notes", "秘密 📝\nSecond line <script>plain text</script>");
        await click("Add custom field");
        await fill("field-name-0", "PIN");
        await fill("field-type-0", "hidden");
        await fill("field-value-0", "PRIVATE-NOTE-FIELD");
        await click("Save");
        await saved();
        const note = [...records.values()].find(
            (record) => record.name === "Private note",
        );
        expect(note).toMatchObject({
            kind: "secureNote",
            type: 0,
            notes: "秘密 📝\nSecond line <script>plain text</script>",
            history: [],
        });
        expect(note).not.toHaveProperty("secret");
        expect(target.innerHTML).not.toContain("秘密");
        expect(target.innerHTML).not.toContain("PRIVATE-NOTE-FIELD");
        await click("Show Notes");
        expect(target.textContent).toContain(
            "秘密 📝\nSecond line <script>plain text</script>",
        );
        expect(target.querySelector("script")).toBeNull();
        await click("Edit");
        await fill("notes", "abandoned");
        await fill("field-value-0", "abandoned");
        await click("Cancel");
        expect(target.innerHTML).not.toContain("秘密");
        expect(records.get(note!.id)).toEqual(note);
        expect(records.get(original.id)).toEqual(original);
    });

    it("generates without writes, inserts only on request with confirmation, and preserves history through cancel/save", async () => {
        const { records, vault } = await setup([original, other]);
        await click("Password generator");
        await click("Generate");
        expect(target.textContent).not.toContain("Use in Login draft");
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
        await click(original.name);
        await click("Edit");
        await click("Password generator");
        await click("Generate");
        await click("Show Generated value");
        const generated = button("Hide Generated value")
            .closest("fieldset")
            ?.querySelector("p")?.textContent;
        expect(generated).toHaveLength(20);
        expect(control("secret").value).toBe(original.secret);
        await click("Use in Login draft");
        expect(control("secret").value).toBe(generated);
        expect(control("confirm-secret").value).toBe(generated);
        expect(control("secret").type).toBe("password");
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
        await click("Cancel");
        expect(records.get(original.id)).toEqual(original);
        await click("Edit");
        await click("Password generator");
        await click("Generate");
        await click("Use in Login draft");
        const savedPassword = control("secret").value;
        await click("Save");
        await saved();
        const changed = records.get(original.id)!;
        expect(changed.secret).toBe(savedPassword);
        expect(changed.history).toEqual([
            { secret: original.secret, changedAt: changed.updatedAt },
        ]);
        expect(records.get(other.id)).toEqual(other);
        await click("Edit");
        expect(control("secret").value).toBe(savedPassword);
    });

    it("discards generated state and pending clipboard feedback on navigation and retirement", async () => {
        await setup([original, other]);
        let finish!: () => void;
        const write = vi
            .spyOn(navigator.clipboard, "writeText")
            .mockImplementation(
                () =>
                    new Promise<void>((resolve) => {
                        finish = resolve;
                    }),
            );
        await click("Password generator");
        await click("Generate");
        await click("Copy Generated value");
        expect(write).toHaveBeenCalledOnce();
        await click(other.name);
        finish();
        await tick();
        expect(target.textContent).not.toContain("Copied.");
        await click("Password generator");
        expect(button("Copy Generated value").disabled).toBe(true);
        await click("Generate");
        (component as { retire: () => void }).retire();
        flushSync();
        expect(target.textContent).not.toContain("Generated value");
    });
    it("opens read-only details without writes and reveals only on an explicit action", async () => {
        const { vault, records } = await setup([original, other]);
        await click(original.name);
        expect(target.querySelector("form")).toBeNull();
        expect(target.textContent).toContain("Original notes");
        expect(target.innerHTML).not.toContain("old-password");
        expect(button("Edit")).toBeDefined();
        await click("Show Password");
        expect(target.textContent).toContain("old-password");
        await click(other.name);
        expect(target.innerHTML).not.toContain("old-password");
        await click(original.name);
        expect(target.innerHTML).not.toContain("old-password");
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
        expect(records.get(original.id)).toEqual(original);
    });
    it("reports clipboard success, denied and unavailable access without exposing the value in feedback", async () => {
        await setup([original]);
        const write = vi.fn().mockResolvedValue(undefined);
        vi.spyOn(navigator, "clipboard", "get").mockReturnValue({
            writeText: write,
        } as unknown as Clipboard);
        await click(original.name);
        await click("Copy Password");
        await expect
            .poll(() => target.querySelector('[role="status"]')?.textContent)
            .toContain("Copied.");
        expect(write).toHaveBeenCalledWith("old-password");
        expect(target.innerHTML).not.toContain("old-password");
        write.mockRejectedValueOnce(new Error("old-password"));
        await click("Copy Password");
        await expect
            .poll(() => target.querySelector('[role="alert"]')?.textContent)
            .toContain("Copy failed.");
        expect(target.innerHTML).not.toContain("old-password");
        vi.spyOn(navigator, "clipboard", "get").mockReturnValue(
            undefined as unknown as Clipboard,
        );
        await click("Copy Password");
        await expect
            .poll(() => target.querySelector('[role="alert"]')?.textContent)
            .toContain("unavailable or denied");
    });
    it("protects hidden fields and ignores clipboard completion after item navigation", async () => {
        await setup([
            {
                ...original,
                fields: [{ name: "PIN", type: "hidden", value: "PRIVATE-PIN" }],
            },
            other,
        ]);
        let finish!: () => void;
        const write = vi
            .spyOn(navigator.clipboard, "writeText")
            .mockImplementationOnce(
                () =>
                    new Promise<void>((resolve) => {
                        finish = resolve;
                    }),
            );
        await click(original.name);
        expect(target.innerHTML).not.toContain("PRIVATE-PIN");
        await click("Show Custom field 1: PIN");
        expect(target.textContent).toContain("PRIVATE-PIN");
        await click("Copy Custom field 1: PIN");
        expect(write).toHaveBeenCalledWith("PRIVATE-PIN");
        await click(other.name);
        finish();
        await tick();
        expect(target.textContent).not.toContain("Copied.");
        expect(target.innerHTML).not.toContain("PRIVATE-PIN");
        await click(original.name);
        expect(target.innerHTML).not.toContain("PRIVATE-PIN");
    });
    it("keeps dirty nested edits when navigation is rejected and discards them only on approval", async () => {
        const { records, vault } = await setup([original, other]);
        const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
        await edit(original.name);
        await fill("uri-0", "discarded");
        await fill("field-value-0", "discarded");
        await click(other.name);
        expect(confirm).toHaveBeenCalledWith("Discard unsaved changes?");
        expect(control("uri-0").value).toBe("discarded");
        await click("Refresh");
        expect(control("field-value-0").value).toBe("discarded");
        const leave = new Event("beforeunload", { cancelable: true });
        window.dispatchEvent(leave);
        expect(leave.defaultPrevented).toBe(true);
        confirm.mockReturnValue(true);
        await click(other.name);
        expect(target.querySelector("form")).toBeNull();
        expect(target.querySelector("[data-detail-heading]")?.textContent).toBe(
            other.name,
        );
        expect(document.activeElement).toBe(
            target.querySelector("[data-detail-heading]"),
        );
        await edit(original.name);
        expect(control("uri-0").value).toBe(original.uris[0].uri);
        expect(records.get(original.id)).toEqual(original);
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
    });
    it("saves and reopens a username-only Login without password confirmation", async () => {
        const { records } = await setup();
        await click("New secret");
        await fill("name", "Passwordless login");
        await fill("username", "alice");
        expect((control("secret") as HTMLInputElement).required).toBe(false);
        expect(target.querySelector("#confirm-secret")).toBeNull();
        await click("Save");
        await saved();
        expect([...records.values()][0]).toMatchObject({
            kind: "login",
            username: "alice",
        });
        await edit("Passwordless login");
        expect(control("secret").value).toBe("");
        expect(control("username").value).toBe("alice");
    });
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
        expect(target.textContent).toContain("New login");
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
        await edit("New login");
        expect(control("username").value).toBe("alice");
        expect(control("uri-1").value).toBe(record.uris[1]?.uri);
        expect(control("match-1").value).toBe("startsWith");
        expect(control("field-value-2").value).toBe("false");
    });

    it("edits both websites and match rules and saves the same record", async () => {
        const { records, vault } = await setup([original, other]);
        await edit(original.name);
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
        expect(target.textContent).toContain("Personal login");
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
        expect(target.textContent).toContain("Confirmed login");
        // Get the new record's ID
        const newRecord = [...records.values()].find(
            (r) => r.name === "Confirmed login",
        );
        if (!newRecord) throw new Error("Expected Confirmed login record");
        await edit("Confirmed login");
        await expect.poll(() => target.querySelector("form")).not.toBeNull();
        expect((control("secret") as HTMLInputElement).type).toBe("password");
        expect(target.querySelector("#confirm-secret")).toBeNull();
        await fill("notes", "Notes only");
        await click("Save");
        await saved();
        expect(records.get(newRecord.id)?.notes).toBe("Notes only");
        expect(target.textContent).toContain("Confirmed login");
        await edit("Confirmed login");
        await expect.poll(() => target.querySelector("form")).not.toBeNull();
        await fill("secret", "second-password");
        await fill("confirm-secret", "wrong-password");
        await click("Save");
        expect(records.get(newRecord.id)?.secret).toBe("first-password");
        await fill("confirm-secret", "second-password");
        await click("Save");
        await saved();
        expect(records.get(newRecord.id)?.secret).toBe("second-password");
        expect(target.textContent).toContain("Confirmed login");
        await edit("Confirmed login");
        await expect.poll(() => target.querySelector("form")).not.toBeNull();
        await fill("secret", "third-password");
        expect(control("confirm-secret").value).toBe("");
        await fill("confirm-secret", "third-password");
        await click("Save");
        await saved();
        expect(records.get(newRecord.id)?.secret).toBe("third-password");
        expect(target.textContent).toContain("Confirmed login");
        expect(records.get(other.id)).toEqual(other);
        expect(JSON.stringify([...records.values()])).not.toContain(
            "confirmPassword",
        );
    });

    it("Cancel discards nested edits without changing the collection or persisted record", async () => {
        const { records, vault } = await setup([original]);
        await edit(original.name);
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

        await edit(original.name);
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

    it.each(["trash", "restore", "purge"] as const)(
        "does not claim success for an unacknowledged %s",
        async (operation) => {
            const initial =
                operation === "trash"
                    ? original
                    : { ...original, deletedAt: "2026-01-01T00:00:00.000Z" };
            const { vault, records } = await setup([initial, other]);
            if (operation !== "trash") await click("Trash");
            if (operation === "purge") {
                vault.deleteSecret.mockImplementationOnce(async () => {});
                await click("Delete permanently");
                await click("Confirm permanent delete");
            } else {
                vault.encryptAndStore.mockImplementationOnce(async () => ({
                    id: initial.id,
                    ciphertext: JSON.stringify(initial),
                    nonce: "test",
                    createdAt: initial.createdAt,
                    updatedAt: initial.updatedAt,
                }));
                if (operation === "trash") {
                    await click("Delete");
                    await click("Confirm delete");
                } else await click("Restore");
            }
            await expect
                .poll(() => target.textContent)
                .toContain(
                    "Storage write was not acknowledged; authoritative records reloaded. Verify the outcome before retrying.",
                );
            expect(records.get(original.id)).toEqual(initial);
            expect(records.get(other.id)).toEqual(other);
            expect(target.textContent).not.toContain(
                operation === "trash"
                    ? "Moved to Trash."
                    : operation === "restore"
                      ? "Restored."
                      : "Permanently deleted.",
            );
            if (operation === "trash")
                expect(button(original.name)).toBeDefined();
            else expect(button("Restore")).toBeDefined();
        },
    );

    it("moves only the selected item to Trash, restores it intact, and separately confirms permanent deletion", async () => {
        const { records, vault } = await setup([original, other]);
        await edit(original.name);
        await click("Delete");
        expect(target.textContent).toContain(`Move ${original.name} to Trash?`);
        expect(records.get(original.id)).toEqual(original);
        expect(vault.deleteSecret).not.toHaveBeenCalled();
        await click("Cancel delete");
        expect(target.textContent).not.toContain("This cannot be undone.");
        expect(target.querySelector("form")).toBeNull();
        expect(vault.deleteSecret).not.toHaveBeenCalled();
        await click("Delete");
        await click("Confirm delete");
        await expect
            .poll(() => target.textContent)
            .toContain("Moved to Trash.");
        expect(vault.deleteSecret).not.toHaveBeenCalled();
        expect(records.get(original.id)).toEqual({
            ...original,
            deletedAt: expect.any(String),
        });
        expect(records.get(other.id)).toEqual(other);
        expect(target.querySelector("form")).toBeNull();
        expect(target.textContent).not.toContain(original.name);
        expect(button(other.name)).toBeDefined();
        await click("Refresh");
        await expect.poll(() => target.textContent).toContain("Refreshed.");
        expect(target.textContent).not.toContain(original.name);
        expect(button(other.name)).toBeDefined();
        await click("Trash");
        expect(button("Trash").getAttribute("aria-pressed")).toBe("true");
        expect(target.textContent).toContain(original.name);
        expect(target.textContent).not.toContain(other.name);
        expect(target.querySelector("form")).toBeNull();
        await click("Restore");
        await expect.poll(() => target.textContent).toContain("Restored.");
        expect(records.get(original.id)).toEqual(original);
        expect(target.textContent).toContain("Trash is empty.");
        await click("All items");
        await click("Delete");
        await click("Confirm delete");
        await expect
            .poll(() => target.textContent)
            .toContain("Moved to Trash.");
        await click("Trash");
        await click("Delete permanently");
        expect(target.querySelector('[role="alertdialog"]')).not.toBeNull();
        expect(document.activeElement).toBe(button("Cancel delete"));
        expect(vault.deleteSecret).not.toHaveBeenCalled();
        await click("Cancel delete");
        expect(document.activeElement).toBe(button("Delete permanently"));
        expect(records.has(original.id)).toBe(true);
        await click("Delete permanently");
        await click("Confirm permanent delete");
        await expect
            .poll(() => target.textContent)
            .toContain("Permanently deleted.");
        expect(vault.deleteSecret).toHaveBeenCalledExactlyOnceWith(original.id);
        expect(records.has(original.id)).toBe(false);
        expect(records.get(other.id)).toEqual(other);
        expect(target.textContent).toContain("Trash is empty.");
    });
});
