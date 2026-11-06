import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { EncryptedSecret, VaultHandle } from "../vault/vault";
import LocalSecretEditor from "./LocalSecretEditor.svelte";
import type { Secret } from "./secret";

const date = "2026-01-01T00:00:00.000Z";
function login(
    name: string,
    extra: Partial<Extract<Secret, { kind: "login" }>> = {},
): Secret {
    return {
        kind: "login",
        id: crypto.randomUUID(),
        name,
        favorite: false,
        fields: [],
        history: [],
        uris: [],
        createdAt: date,
        updatedAt: date,
        ...extra,
    };
}
let target: HTMLDivElement;
let component: (ReturnType<typeof mount> & { retire(): void }) | undefined;
function button(name: string) {
    const result = [...target.querySelectorAll("button")].find(
        (element) => element.textContent?.trim() === name,
    );
    if (!result) throw new Error(`Missing button: ${name}`);
    return result;
}
async function click(name: string) {
    const element = button(name);
    expect(element.disabled).toBe(false);
    element.click();
    flushSync();
    await tick();
}
function control(label: string) {
    const element = [...target.querySelectorAll("input, select")].find(
        (element) => {
            const linked = (element as HTMLInputElement).labels?.[0];
            return (
                (
                    linked?.querySelector(".field-label-text") ?? linked
                )?.textContent?.trim() === label
            );
        },
    );
    if (!element) throw new Error(`Missing control: ${label}`);
    return element as HTMLInputElement | HTMLSelectElement;
}
async function fill(label: string, value: string | boolean) {
    const element = control(label);
    expect(element.disabled).toBe(false);
    if (typeof value === "boolean")
        (element as HTMLInputElement).checked = value;
    else element.value = value;
    element.dispatchEvent(
        new Event(
            element instanceof HTMLSelectElement || typeof value === "boolean"
                ? "change"
                : "input",
            { bubbles: true },
        ),
    );
    flushSync();
    await tick();
}
function names() {
    return [
        ...target.querySelectorAll('ul[aria-label="Search results"] li'),
    ].map((element) => element.querySelector("button")?.textContent?.trim());
}
async function setup(initial: Secret[] = []) {
    const records = new Map(
        initial.map((record) => [record.id, structuredClone(record)]),
    );
    const encrypted = (record: Secret): EncryptedSecret => ({
        id: record.id,
        ciphertext: JSON.stringify(record),
        nonce: "test",
        createdAt: record.createdAt,
        updatedAt: record.updatedAt,
    });
    const vault = {
        id: crypto.randomUUID(),
        readItems: vi.fn(async () => ({
            items: [...records.values()].map((record) =>
                structuredClone(record),
            ),
            folders: [],
        })),
        encryptAndStore: vi.fn(async (plaintext: string) => {
            const record: Secret = JSON.parse(plaintext);
            records.set(record.id, record);
            return encrypted(record);
        }),
        deleteSecret: vi.fn(async (id: string) => {
            records.delete(id);
        }),
        close: vi.fn(),
    };
    target = document.createElement("div");
    document.body.append(target);
    component = mount(LocalSecretEditor, {
        target,
        props: { vault: vault as unknown as VaultHandle },
    });
    flushSync();
    await expect.poll(() => button("New secret").disabled).toBe(false);
    return { records, vault, encrypted };
}
afterEach(async () => {
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
    vi.restoreAllMocks();
});

describe("unlocked search through the real workflow and collection", () => {
    it("combines text, type and favorite filters, clears them and opens read-only details", async () => {
        const favorite = login("Travel", {
            favorite: true,
            notes: "Train booking",
        });
        const { vault, records } = await setup([
            favorite,
            login("Other", { notes: "Train booking" }),
            login("Unrelated", { favorite: true }),
        ]);
        await fill("Search items", "TRAIN");
        await fill("Item type", "login");
        await fill("Favorites only", true);
        expect(names()).toEqual(["★ Travel"]);
        await click("★ Travel");
        expect(target.textContent).toContain("Train booking");
        expect(target.querySelector("form")).toBeNull();
        await fill("Search items", "no match");
        expect(names()).toEqual([]);
        expect(target.querySelector('[role="status"]')?.textContent).toContain(
            "No items match",
        );
        await click("Clear filters");
        expect(control("Search items").value).toBe("");
        expect(control("Item type").value).toBe("all");
        expect((control("Favorites only") as HTMLInputElement).checked).toBe(
            false,
        );
        expect(names()).toEqual(["★ Travel", "★ Unrelated", "Other"]);
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
        expect(vault.deleteSecret).not.toHaveBeenCalled();
        expect(records.get(favorite.id)).toEqual(favorite);
    });

    it("searches only allowed fields across every defined kind, with Unicode and missing data", async () => {
        const base = {
            createdAt: date,
            updatedAt: date,
            favorite: false,
            fields: [],
            history: [],
        };
        const items: Secret[] = [
            login("Cafe\u0301 account", {
                username: "User_One",
                uris: [{ uri: "https://host.test/path", match: "never" }],
                secret: "password-needle",
                totp: "totp-needle",
                history: [{ secret: "history-needle", changedAt: date }],
                fields: [
                    {
                        name: "Label-needle",
                        type: "hidden",
                        value: "value-needle",
                    },
                ],
            }),
            {
                ...base,
                kind: "secureNote",
                id: crypto.randomUUID(),
                name: "Note",
                notes: "日本語 memo",
                type: 0,
            },
            {
                ...base,
                kind: "card",
                id: crypto.randomUUID(),
                name: "Card",
                number: "number-needle",
            },
            {
                ...base,
                kind: "identity",
                id: crypto.randomUUID(),
                name: "Identity",
                username: "identity-user",
                ssn: "ssn-needle",
            },
            {
                ...base,
                kind: "sshKey",
                id: crypto.randomUUID(),
                name: "Key",
                privateKey: "private-needle",
                publicKey: "public-needle",
            },
            login("Deleted", { notes: "日本語 memo", deletedAt: date }),
        ];
        await setup(items);
        for (const [text, expected] of [
            [" CAFÉ ", "Cafe\u0301 account"],
            ["USER_ONE", "Cafe\u0301 account"],
            ["HOST.TEST/PATH", "Cafe\u0301 account"],
            ["label-NEEDLE", "Cafe\u0301 account"],
            ["日本語", "Note"],
            ["IDENTITY-USER", "Identity"],
        ]) {
            await fill("Search items", text);
            expect(names()).toEqual([expected]);
        }
        for (const text of [
            "password-needle",
            "totp-needle",
            "history-needle",
            "value-needle",
            "number-needle",
            "ssn-needle",
            "private-needle",
            "public-needle",
        ]) {
            await fill("Search items", text);
            expect(names()).toEqual([]);
        }
        await click("Clear filters");
        for (const [kind, name] of [
            ["login", "Cafe\u0301 account"],
            ["secureNote", "Note"],
            ["card", "Card"],
            ["identity", "Identity"],
            ["sshKey", "Key"],
        ]) {
            await fill("Item type", kind);
            expect(names()).toEqual([name]);
        }
    });

    it("uses explicit sort orders with stable ID ties and leaves the source unchanged", async () => {
        const alpha = login("Alpha", {
            favorite: true,
            createdAt: "2026-01-03T00:00:00Z",
            updatedAt: "2026-01-02T00:00:00Z",
        });
        const beta = login("Beta", { updatedAt: "2026-01-04T00:00:00Z" });
        const zeta = login("Zeta", { createdAt: "2026-01-02T00:00:00Z" });
        const { records } = await setup([zeta, beta, alpha]);
        expect(names()).toEqual(["★ Alpha", "Beta", "Zeta"]);
        for (const [sort, expected] of [
            ["nameAsc", ["★ Alpha", "Beta", "Zeta"]],
            ["nameDesc", ["Zeta", "Beta", "★ Alpha"]],
            ["updatedDesc", ["Beta", "★ Alpha", "Zeta"]],
            ["createdDesc", ["★ Alpha", "Zeta", "Beta"]],
        ] as const) {
            await fill("Sort items", sort);
            expect(names()).toEqual(expected);
        }
        expect([...records.values()]).toEqual([zeta, beta, alpha]);
        const first = login("Equal", {
            id: "12345678-1234-4234-8234-123456789abc",
            username: "first",
        });
        const second = login("Equal", {
            id: "12345678-1234-4234-8234-123456789abd",
            username: "second",
        });
        records.clear();
        records.set(second.id, second);
        records.set(first.id, first);
        await click("Refresh");
        await expect.poll(() => target.textContent).toContain("Refreshed.");
        expect(
            [
                ...target.querySelectorAll(
                    'ul[aria-label="Search results"] li',
                ),
            ].map((element) => element.querySelector("p")?.textContent),
        ).toEqual(["first", "second"]);
    });

    it("keeps combined results current after acknowledged create, edit, delete and refresh", async () => {
        const { records } = await setup();
        expect(target.textContent).toContain("No secrets yet.");
        await fill("Search items", "match");
        await fill("Favorites only", true);
        await click("New secret");
        const setDraft = async (id: string, value: string) => {
            const input = target.querySelector<HTMLInputElement>(`#${id}`);
            if (!input) throw new Error(`Missing draft control: ${id}`);
            input.value = value;
            input.dispatchEvent(new Event("input", { bubbles: true }));
            flushSync();
            await tick();
        };
        await setDraft("name", "Match created");
        target.querySelector<HTMLInputElement>("#favorite")?.click();
        flushSync();
        await click("Save");
        await expect.poll(() => target.textContent).toContain("Saved.");
        expect(names()).toEqual(["★ Match created"]);
        await click("Edit");
        await setDraft("name", "Renamed");
        await click("Save");
        await expect.poll(() => target.textContent).toContain("Saved.");
        expect(names()).toEqual([]);
        await click("Clear filters");
        expect(names()).toEqual(["★ Renamed"]);
        await click("Delete");
        await click("Confirm delete");
        await expect
            .poll(() => target.textContent)
            .toContain("Moved to Trash.");
        expect(names()).toEqual([]);
        expect(records.size).toBe(1);
        expect([...records.values()][0].deletedAt).toEqual(expect.any(String));
        const external = login("Match external", { favorite: true });
        records.set(external.id, external);
        await fill("Search items", "match");
        await fill("Favorites only", true);
        await click("Refresh");
        await expect.poll(names).toEqual(["★ Match external"]);
        records.set(external.id, { ...external, favorite: false });
        await click("Refresh");
        await expect.poll(names).toEqual([]);
    });

    it("clears results and query on retirement and ignores a late refresh", async () => {
        const item = login("Sensitive query result");
        const { vault, encrypted } = await setup([item]);
        await fill("Search items", "Sensitive query");
        await click(item.name);
        let finish: (snapshot: { items: Secret[]; folders: [] }) => void =
            () => {};
        vault.readItems.mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    finish = resolve;
                }),
        );
        await click("Refresh");
        expect(target.textContent).toContain("Refreshing secrets");
        component?.retire();
        flushSync();
        await tick();
        expect(names()).toEqual([]);
        expect(target.innerHTML).not.toContain("Sensitive query");
        expect(target.querySelector('input[type="search"]')).toBeNull();
        finish({ items: [item], folders: [] });
        await tick();
        flushSync();
        expect(names()).toEqual([]);
        expect(target.innerHTML).not.toContain("Sensitive query");
        expect(button("New secret").disabled).toBe(true);
    });

    it("hides stale results on a read error and retains filters for a successful retry", async () => {
        const { vault } = await setup([login("Retry result")]);
        await fill("Search items", "retry");
        vault.readItems.mockRejectedValueOnce(
            new Error("Synthetic read failure"),
        );
        await click("Refresh");
        await expect
            .poll(() => target.querySelector('[role="alert"]')?.textContent)
            .toContain("Synthetic read failure");
        expect(names()).toEqual([]);
        expect(control("Search items").matches(":disabled")).toBe(true);
        expect(target.textContent).not.toContain("No items match");
        await click("Refresh");
        await expect.poll(names).toEqual(["Retry result"]);
        expect(control("Search items").value).toBe("retry");
    });

    it("filters a representative 2,000-item synthetic vault without a stored index", async () => {
        const items = Array.from({ length: 2000 }, (_, index) =>
            login(`Item ${String(index).padStart(4, "0")}`, {
                favorite: index % 100 === 0,
                notes: index % 2 === 0 ? "Even" : undefined,
            }),
        );
        const { vault } = await setup(items);
        const local = JSON.stringify(localStorage);
        const session = JSON.stringify(sessionStorage);
        const location = window.location.href;
        const databases = await indexedDB.databases();
        await fill("Search items", "EVEN");
        await fill("Favorites only", true);
        await fill("Item type", "login");
        await fill("Sort items", "nameDesc");
        expect(names()).toHaveLength(20);
        expect(names()[0]).toBe("★ Item 1900");
        expect(names()[19]).toBe("★ Item 0000");
        expect(JSON.stringify(localStorage)).toBe(local);
        expect(JSON.stringify(sessionStorage)).toBe(session);
        expect(await indexedDB.databases()).toEqual(databases);
        expect(window.location.href).toBe(location);
        expect(vault.encryptAndStore).not.toHaveBeenCalled();
        expect(vault.deleteSecret).not.toHaveBeenCalled();
    });
});
