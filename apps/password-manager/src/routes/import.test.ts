import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import supported from "../../../../docs/fixtures/password-manager/supported.json";
import { createVault, unlockVault, VaultHandle } from "$lib/vault/vault";
import Page from "./+page.svelte";

const password = "synthetic-import-page";
let target: HTMLDivElement;
let component: ReturnType<typeof mount> | undefined;
function button(name: string) {
    const element = [...target.querySelectorAll("button")].find(
        (element) =>
            (
                element.querySelector(".vault-name")?.textContent ??
                element.textContent
            )
                ?.trim()
                .replace(/^★ /, "") === name,
    );
    if (!element) throw new Error(`Missing button: ${name}`);
    return element;
}
async function click(name: string) {
    expect(button(name).disabled).toBe(false);
    button(name).click();
    flushSync();
    await tick();
}
function file(content: string) {
    const element = target.querySelector<HTMLInputElement>("#import-file")!;
    const transfer = new DataTransfer();
    transfer.items.add(
        new File([content], "synthetic.json", { type: "application/json" }),
    );
    element.files = transfer.files;
    element.dispatchEvent(new Event("change", { bubbles: true }));
    flushSync();
}
async function unlock(name: string) {
    await expect.poll(() => target.textContent).toContain(name);
    await click(name);
    const input = target.querySelector<HTMLInputElement>("#password")!;
    input.value = password;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    await click("Unlock");
    await expect.poll(() => button("New secret").disabled).toBe(false);
}
async function start(name: string) {
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await unlock(name);
}
async function preview(content = JSON.stringify(supported)) {
    await click("Import Bitwarden JSON");
    file(content);
    await click("Validate import");
    await expect.poll(() => target.textContent).toContain("Import preview");
}
afterEach(async () => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
});

it("previews, cancels, imports all five types, reopens, and confirms repeat-as-new through the real page", async () => {
    const name = `Import page ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    const id = vault.id;
    vault.close();
    await start(name);
    await preview();
    expect(target.textContent).toContain("Plaintext files expose credentials");
    expect(target.textContent).toContain("Login: 1");
    expect(target.textContent).toContain("Folders: 2");
    expect(target.textContent).toContain("date unavailable");
    expect(target.textContent).not.toContain("SYNTHETIC-NOT-A-CREDENTIAL");
    expect(target.textContent).toContain(id);
    await click("Cancel import");
    const reader = await unlockVault(password, id);
    expect((await reader.readItems()).items).toEqual([]);
    reader.close();
    await preview();
    await click("Import all as new");
    await expect.poll(() => target.textContent).toContain("Import verified");
    await expect
        .poll(() => target.textContent)
        .toContain(supported.items[0].name);
    await click(supported.items[0].name);
    expect(target.textContent).toContain("Password history");
    await click("Delete");
    await click("Confirm delete");
    await expect.poll(() => target.textContent).toContain("Moved to Trash.");
    await click("Trash");
    expect(target.textContent).toContain(supported.items[0].name);
    await click("Restore");
    await expect.poll(() => target.textContent).toContain("Restored.");
    await click("All items");
    expect(target.textContent).not.toContain("SYNTHETIC-PRIOR-ONE");
    const folders = [
        ...target.querySelectorAll<HTMLOptionElement>(
            '[aria-label="Folder filter"] option',
        ),
    ];
    expect(
        folders.filter((option) =>
            option.textContent?.startsWith("Synthetic/日本語"),
        ),
    ).toHaveLength(1);
    await click("Lock vault");
    await unlock(name);
    await expect
        .poll(() => target.textContent)
        .toContain(supported.items[4].name);
    await preview();
    expect(target.textContent).toContain("Source-ID matches: 5");
    expect(target.textContent).toContain("Logical duplicates: 5");
    expect(button("Import all as new").disabled).toBe(true);
    const confirm = target.querySelector<HTMLInputElement>(
        '[aria-label="Import duplicates as new"]',
    )!;
    confirm.click();
    flushSync();
    await click("Import all as new");
    await expect.poll(() => target.textContent).toContain("Import verified");
    const final = await unlockVault(password, id);
    expect((await final.readItems()).items).toHaveLength(10);
    final.close();
});

it("blocks unknown nested mappings without leaking values or writing supported records", async () => {
    const name = `Import invalid ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    const id = vault.id;
    vault.close();
    await start(name);
    const source: any = structuredClone(supported);
    source.items[0].login.uris[0].uriChecksum = "PRIVATE-UNKNOWN";
    source.items[1].permissions = { value: "PRIVATE-UNKNOWN" };
    source.items[2].card.future = "PRIVATE-UNKNOWN";
    await preview(JSON.stringify(source));
    expect(target.textContent).toContain("Import blocked");
    for (const path of [
        "/items/0/login/uris/0/uriChecksum",
        "/items/1/permissions",
        "/items/2/card/future",
    ])
        expect(target.textContent).toContain(path);
    expect(target.innerHTML).not.toContain("PRIVATE-UNKNOWN");
    expect(
        [...target.querySelectorAll("button")].some(
            (element) => element.textContent === "Import all as new",
        ),
    ).toBe(false);
    await click("Cancel import");
    const reader = await unlockVault(password, id);
    expect((await reader.readItems()).items).toEqual([]);
    reader.close();
});

it("keeps dirty drafts on refused import navigation and restores focus after cancel", async () => {
    const name = `Import draft ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    vault.close();
    await start(name);
    await click("New secret");
    const input = target.querySelector<HTMLInputElement>("#name")!;
    input.value = "PRIVATE-DRAFT";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    await click("Import Bitwarden JSON");
    expect(confirm).toHaveBeenCalled();
    expect(target.querySelector<HTMLInputElement>("#name")?.value).toBe(
        "PRIVATE-DRAFT",
    );
    confirm.mockReturnValue(true);
    await click("Import Bitwarden JSON");
    expect(target.querySelector("#name")).toBeNull();
    expect(document.activeElement?.textContent).toBe("Import Bitwarden JSON");
    await click("Cancel import");
    await expect.poll(() => button("New secret").disabled).toBe(false);
    expect(document.activeElement).toBe(button("Import Bitwarden JSON"));
});

it.each(["quota", "uncertain"])(
    "reports %s import failure without false success and reloads the authoritative batch",
    async (phase) => {
        const name = `Import UI failure ${phase} ${crypto.randomUUID()}`;
        const vault = await createVault(password, name);
        vault.close();
        await start(name);
        await preview();
        const put = IDBObjectStore.prototype.put;
        const getAll = IDBObjectStore.prototype.getAll;
        let published = false;
        const write = vi
            .spyOn(IDBObjectStore.prototype, "put")
            .mockImplementation(function (this: IDBObjectStore, value, key) {
                if (this.name === "records" && phase === "quota")
                    throw new DOMException(
                        "PRIVATE-FAULT",
                        "QuotaExceededError",
                    );
                if (this.name === "vaults" && phase === "uncertain")
                    published = true;
                return put.call(this, value, key);
            });
        const read = vi
            .spyOn(IDBObjectStore.prototype, "getAll")
            .mockImplementation(function (this: IDBObjectStore, ...args) {
                if (this.name === "vaults" && published)
                    throw new DOMException("PRIVATE-FAULT", "UnknownError");
                return getAll.apply(this, args);
            });
        await click("Import all as new");
        await expect
            .poll(() => target.textContent)
            .toContain("Import could not be confirmed");
        expect(target.textContent).not.toContain("Import verified");
        expect(target.innerHTML).not.toContain("PRIVATE-FAULT");
        write.mockRestore();
        read.mockRestore();
        await click("Lock vault");
        await unlock(name);
        if (phase === "uncertain")
            await expect
                .poll(() => target.textContent)
                .toContain(supported.items[4].name);
        else expect(target.textContent).toContain("No secrets yet.");
    },
);

for (const mode of ["manual", "automatic"] as const) {
    it.each(["file", "read", "encryption", "write", "acknowledgement"])(
        `retires late %s work after ${mode} lock without late feedback`,
        async (phase) => {
            const name = `Import lock ${mode} ${phase} ${crypto.randomUUID()}`;
            const vault = await createVault(password, name);
            await vault.saveLockSettings({ timeoutMinutes: 1 });
            const id = vault.id;
            vault.close();
            await start(name);
            let release!: () => void;
            const gate = new Promise<void>((resolve) => {
                release = resolve;
            });
            let started = false;
            let finished = false;
            if (phase === "file" || phase === "read") {
                await click("Import Bitwarden JSON");
                file(JSON.stringify(supported));
                if (phase === "file") {
                    const original = File.prototype.arrayBuffer;
                    vi.spyOn(
                        File.prototype,
                        "arrayBuffer",
                    ).mockImplementationOnce(async function (this: File) {
                        const result = await original.call(this);
                        started = true;
                        await gate;
                        finished = true;
                        return result;
                    });
                } else {
                    const original = VaultHandle.prototype.readItems;
                    vi.spyOn(
                        VaultHandle.prototype,
                        "readItems",
                    ).mockImplementationOnce(async function (
                        this: VaultHandle,
                    ) {
                        const result = await original.call(this);
                        started = true;
                        await gate;
                        finished = true;
                        return result;
                    });
                }
                await click("Validate import");
            } else if (phase === "encryption") {
                await preview();
                const original = crypto.subtle.encrypt.bind(crypto.subtle);
                vi.spyOn(crypto.subtle, "encrypt").mockImplementationOnce(
                    async (...args) => {
                        const result = await original(...args);
                        started = true;
                        await gate;
                        finished = true;
                        return result;
                    },
                );
                await click("Import all as new");
            } else {
                await preview();
                const original = VaultHandle.prototype.mutateItems;
                vi.spyOn(
                    VaultHandle.prototype,
                    "mutateItems",
                ).mockImplementationOnce(async function (
                    this: VaultHandle,
                    mutation,
                ) {
                    if (phase === "acknowledgement")
                        await original.call(this, mutation);
                    started = true;
                    await gate;
                    try {
                        if (phase === "write")
                            await original.call(this, mutation);
                    } finally {
                        finished = true;
                    }
                });
                await click("Import all as new");
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
                .toContain("Vault locked.");
            expect(target.textContent).not.toContain("Import preview");
            release();
            await expect.poll(() => finished).toBe(true);
            await tick();
            expect(target.textContent).not.toContain("Import verified");
            expect(target.textContent).not.toContain(
                "Import could not be confirmed",
            );
            expect(target.textContent).not.toContain(supported.items[0].name);
            vi.restoreAllMocks();
            const reader = await unlockVault(password, id);
            expect((await reader.readItems()).items).toHaveLength(
                phase === "acknowledgement" ? 5 : 0,
            );
            reader.close();
        },
        20000,
    );
}
