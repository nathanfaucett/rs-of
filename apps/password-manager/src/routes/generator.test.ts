import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import { prepareBackupRestore } from "$lib/vault/backup";
import { createVault, unlockVault } from "$lib/vault/vault";
import type { Login } from "$lib/secrets/secret";
import Page from "./+page.svelte";

let target: HTMLDivElement;
let component: ReturnType<typeof mount> | undefined;
const master = "synthetic-generator-master-14";
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
function input(id: string) {
    const found = target.querySelector<HTMLInputElement>(`#${id}`);
    if (!found) throw new Error(`Missing input: ${id}`);
    return found;
}
function fill(id: string, value: string) {
    input(id).value = value;
    input(id).dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
}
async function records(id: string) {
    const vault = await unlockVault(master, id);
    try {
        return await Promise.all(
            (await vault.loadSecrets()).map(
                async (record) =>
                    JSON.parse(await vault.decryptSecret(record)) as Login,
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

it("keeps abandoned generated values out of storage, then saves/reopens and restores the exact generated Login and history", async () => {
    const name = `Generator ${crypto.randomUUID()}`;
    const vault = await createVault(master, name);
    const id = vault.id;
    vault.close();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await expect.poll(() => target.textContent).toContain(name);
    async function unlock() {
        await expect.poll(() => target.textContent).toContain(name);
        await click(name);
        fill("password", master);
        await click("Unlock");
        await expect.poll(() => button("New secret").disabled).toBe(false);
    }
    await unlock();
    await click("New secret");
    fill("name", "Synthetic generated Login");
    await click("Password generator");
    await click("Generate");
    await click("Use in Login draft");
    const abandoned = input("secret").value;
    expect(input("confirm-secret").value).toBe(abandoned);
    await click("Cancel");
    expect(await records(id)).toEqual([]);
    await click("New secret");
    fill("name", "Synthetic generated Login");
    await click("Password generator");
    await click("Generate");
    await click("Use in Login draft");
    const first = input("secret").value;
    await click("Save");
    await expect.poll(() => target.textContent).toContain("Saved.");
    expect((await records(id))[0].secret).toBe(first);
    await click("Edit");
    await click("Password generator");
    await click("Generate");
    await click("Use in Login draft");
    const replacement = input("secret").value;
    await click("Save");
    await expect.poll(() => target.textContent).toContain("Saved.");
    const saved = (await records(id))[0];
    expect(saved.secret).toBe(replacement);
    expect(saved.history).toEqual([
        { secret: first, changedAt: saved.updatedAt },
    ]);
    await click("Password generator");
    await click("Generate");
    await click("Show Generated value");
    const unsaved = button("Hide Generated value")
        .closest("fieldset")
        ?.querySelector("p")?.textContent;
    await click("Lock vault");
    expect(target.textContent).not.toContain("Generated value");
    expect(target.innerHTML).not.toContain(unsaved);
    await unlock();
    await click("Synthetic generated Login");
    expect(target.innerHTML).not.toContain(replacement);
    expect(await records(id)).toEqual([saved]);
    await click("Lock vault");
    const source = await unlockVault(master, id);
    const backup = await source.exportBackup();
    source.close();
    for (const value of [abandoned, first, replacement, unsaved!])
        expect(backup).not.toContain(value);
    if (component) await unmount(component);
    component = undefined;
    target.remove();
    for (const { name } of await indexedDB.databases()) {
        if (!name) continue;
        await new Promise<void>((resolve, reject) => {
            const request = indexedDB.deleteDatabase(name);
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
            request.onblocked = () =>
                reject(new Error("Open synthetic test database"));
        });
    }
    await (await prepareBackupRestore(master, backup)).restore();
    expect(await records(id)).toEqual([saved]);
}, 30000);
