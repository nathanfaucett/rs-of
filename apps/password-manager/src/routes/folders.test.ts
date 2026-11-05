import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import { createVault, unlockVault } from "$lib/vault/vault";
import Page from "./+page.svelte";

const password = "synthetic-folder-ui";
let target: HTMLDivElement;
let component: ReturnType<typeof mount> | undefined;
function button(name: string) {
    const element = [...target.querySelectorAll("button")].find(
        (el) =>
            (
                el.querySelector(".vault-name")?.textContent ?? el.textContent
            )?.trim() === name,
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
function fill(label: string, value: string) {
    const el =
        target.querySelector<HTMLInputElement | HTMLSelectElement>(
            `[aria-label="${label}"]`,
        ) ??
        [
            ...target.querySelectorAll<HTMLInputElement | HTMLSelectElement>(
                "input, select",
            ),
        ].find(
            (element) =>
                (
                    element.labels?.[0]?.querySelector(".field-label-text") ??
                    element.labels?.[0]
                )?.textContent?.trim() === label,
        );
    if (!el) throw new Error(`Missing control: ${label}`);
    el.value = value;
    el.dispatchEvent(
        new Event(el.tagName === "SELECT" ? "change" : "input", {
            bubbles: true,
        }),
    );
    flushSync();
}
afterEach(async () => {
    vi.restoreAllMocks();
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
});

it("creates, cancels, renames, filters, moves and unfiles through the real page, then locks and reopens", async () => {
    const name = `Folder UI ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    await vault.prepareItems();
    const item = {
        kind: "login",
        id: crypto.randomUUID(),
        name: "Synthetic folder Login",
        favorite: false,
        fields: [],
        history: [],
        uris: [],
        createdAt: "2026-01-02T03:04:05.000Z",
        updatedAt: "2026-01-02T03:04:05.000Z",
    };
    await vault.encryptAndStore(JSON.stringify(item), item.id);
    const id = vault.id;
    vault.close();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await expect.poll(() => target.textContent).toContain(name);
    await click(name);
    const master = target.querySelector<HTMLInputElement>("#password")!;
    master.value = password;
    master.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    await click("Unlock");
    await expect.poll(() => button("New secret").disabled).toBe(false);
    await click("New folder");
    expect(document.activeElement).toBe(
        target.querySelector('[aria-label="Folder name"]'),
    );
    fill("Folder name", "Abandoned");
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    fill("Folder filter", "unfiled");
    expect(confirm).toHaveBeenCalled();
    expect(
        target.querySelector<HTMLSelectElement>('[aria-label="Folder filter"]')
            ?.value,
    ).toBe("all");
    expect(target.querySelector('[aria-label="Folder name"]')).not.toBeNull();
    await click("Cancel folder");
    expect(document.activeElement).toBe(
        target.querySelector('[aria-label="Folder filter"]'),
    );
    confirm.mockRestore();
    await click("New folder");
    fill("Folder name", "Work/日本語");
    await click("Save folder");
    await expect.poll(() => target.textContent).toContain("Folder created.");
    await click("New folder");
    fill("Folder name", "Work/日本語");
    await click("Save folder");
    await expect.poll(() => button("New folder").disabled).toBe(false);
    const duplicateOptions = [
        ...target.querySelectorAll<HTMLOptionElement>(
            '[aria-label="Folder filter"] option',
        ),
    ].filter((option) => option.textContent?.startsWith("Work/日本語 — "));
    expect(duplicateOptions).toHaveLength(2);
    expect(duplicateOptions[0].value).not.toBe(duplicateOptions[1].value);
    fill("Folder filter", "unfiled");
    await click(item.name);
    const reader = await unlockVault(password, id);
    const folder = (await reader.readItems()).folders[0];
    reader.close();
    fill("Move to folder", folder.id);
    const put = IDBObjectStore.prototype.put;
    const fault = vi
        .spyOn(IDBObjectStore.prototype, "put")
        .mockImplementation(function (this: IDBObjectStore, value, key) {
            if (this.name === "records")
                throw new DOMException("Synthetic quota", "QuotaExceededError");
            return put.call(this, value, key);
        });
    await click("Move item");
    await expect
        .poll(() => target.textContent)
        .toContain("Atomic write could not be confirmed");
    expect(target.textContent).not.toContain("Item moved.");
    fault.mockRestore();
    await expect.poll(() => button("Move item").disabled).toBe(false);
    expect(target.querySelector("ul")?.textContent).toContain(item.name);
    fill("Move to folder", folder.id);
    await click("Move item");
    await expect.poll(() => target.textContent).toContain("Item moved.");
    expect(target.querySelector("ul")?.textContent ?? "").not.toContain(
        item.name,
    );
    fill("Folder filter", folder.id);
    expect(target.querySelector("ul")?.textContent).toContain(item.name);
    fill("Search items", "synthetic");
    fill("Item type", "secureNote");
    expect(target.querySelector("ul")).toBeNull();
    fill("Item type", "login");
    fill("Sort items", "nameDesc");
    expect(target.querySelector("ul")?.textContent).toContain(item.name);
    await click("Delete");
    await click("Confirm delete");
    await expect.poll(() => target.textContent).toContain("Moved to Trash.");
    expect(target.querySelector("ul")).toBeNull();
    fill("Search items", "no match");
    await click("Trash");
    expect(target.querySelector("ul")?.textContent).toContain(item.name);
    expect(target.querySelector('input[type="search"]')).toBeNull();
    await click("Restore");
    await expect.poll(() => target.textContent).toContain("Restored.");
    await click("All items");
    expect(target.querySelector("ul")).toBeNull();
    fill("Search items", "synthetic");
    expect(target.querySelector("ul")?.textContent).toContain(item.name);
    const retained = await unlockVault(password, id);
    expect((await retained.readItems()).items[0].folderId).toBe(folder.id);
    retained.close();
    await click("Clear filters");
    await click(item.name);
    await click("New folder");
    fill("Folder name", "Unsaved folder");
    const discard = vi.spyOn(window, "confirm").mockReturnValue(false);
    await click("Edit");
    expect(target.querySelector('[aria-label="Folder name"]')).not.toBeNull();
    expect(target.querySelector("#name")).toBeNull();
    discard.mockReturnValue(true);
    await click("Edit");
    expect(target.querySelector('[aria-label="Folder name"]')).toBeNull();
    const itemName = target.querySelector<HTMLInputElement>("#name")!;
    itemName.value = "Unsaved item";
    itemName.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    discard.mockReturnValue(false);
    await click("New folder");
    expect(target.querySelector("#name")).not.toBeNull();
    discard.mockReturnValue(true);
    await click("New folder");
    expect(target.querySelector("#name")).toBeNull();
    await click("Cancel folder");
    discard.mockRestore();
    fill("Move to folder", "");
    await click("Move item");
    await expect.poll(() => target.textContent).toContain("Item moved.");
    expect(target.textContent).toContain("No items in this folder view.");
    fill("Folder filter", "unfiled");
    await click(item.name);
    fill("Move to folder", folder.id);
    await click("Move item");
    await expect.poll(() => target.textContent).toContain("Item moved.");
    fill("Folder filter", folder.id);
    await click("Rename folder");
    fill("Folder name", "Renamed");
    await click("Save folder");
    await expect.poll(() => target.textContent).toContain("Folder renamed.");
    await click("Delete folder");
    await click("Cancel folder");
    await click("Delete folder");
    await click("Confirm folder delete");
    await expect
        .poll(() => target.textContent)
        .toContain("Folder deleted. Items are unfiled.");
    fill("Folder filter", "unfiled");
    expect(target.querySelector("ul")?.textContent).toContain(item.name);
    await click("New folder");
    fill("Folder name", "Unsaved folder must retire");
    await click("Lock vault");
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
    expect(target.textContent).not.toContain(item.name);
    await click(name);
    const input = target.querySelector<HTMLInputElement>("#password")!;
    input.value = password;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    await click("Unlock");
    await expect.poll(() => button(item.name).disabled).toBe(false);
    const reopened = await unlockVault(password, id);
    const saved = await reopened.readItems();
    reopened.close();
    expect(saved.folders).toHaveLength(1);
    expect(saved.folders[0].name).toBe("Work/日本語");
    expect(saved.items).toHaveLength(1);
    expect(saved.items[0].folderId).toBeNull();
}, 20000);
