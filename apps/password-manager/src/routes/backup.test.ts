import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import { createVault, listVaults, unlockVault } from "$lib/vault/vault";
import Page from "./+page.svelte";

let target: HTMLDivElement;
let component: ReturnType<typeof mount>;
async function start() {
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
}
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
function input(id: string, value: string) {
    const element = target.querySelector<HTMLInputElement>(`#${id}`);
    if (!element) throw new Error(`Missing input: ${id}`);
    element.value = value;
    element.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
}
function file(content: string) {
    const element = target.querySelector<HTMLInputElement>("#backup-file");
    if (!element) throw new Error("Missing backup file input");
    const transfer = new DataTransfer();
    transfer.items.add(
        new File([content], "synthetic-backup.json", {
            type: "application/json",
        }),
    );
    element.files = transfer.files;
    element.dispatchEvent(new Event("change", { bubbles: true }));
    flushSync();
}
afterEach(async () => {
    vi.restoreAllMocks();
    if (component) await unmount(component);
    target?.remove();
});

it("downloads an encrypted backup through the page and restores it after source storage is discarded", async () => {
    const password = "synthetic-backup-password";
    const name = `test-backup-ui-${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    const id = crypto.randomUUID();
    const logical = {
        id,
        name: "Synthetic recovered login",
        secret: "SENSITIVE-BACKUP-UI",
        createdAt: "2026-01-02T03:04:05.000Z",
        updatedAt: "2026-01-02T03:04:05.000Z",
    };
    await vault.encryptAndStore(JSON.stringify(logical), id);
    const vaultId = vault.id;
    vault.close();
    await start();
    await click(name);
    input("password", password);
    await click("Unlock");
    await expect
        .poll(() => target.textContent)
        .toContain("Synthetic recovered login");
    let download: Blob | undefined;
    vi.spyOn(URL, "createObjectURL").mockImplementation((blob) => {
        download = blob as Blob;
        return "blob:synthetic-backup";
    });
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    await click("Download encrypted backup");
    await expect.poll(() => download).toBeDefined();
    if (!download) throw new Error("Missing encrypted download");
    const content = await download.text();
    expect(content).not.toContain(logical.secret);
    expect(content).not.toContain(password);
    await expect.poll(() => button("Lock vault").disabled).toBe(false);
    await click("Lock vault");
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
    await unmount(component);
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
    await start();
    await click("Restore encrypted backup");
    file(content);
    input("backup-password", "wrong-password");
    await click("Validate backup");
    await expect
        .poll(() => target.textContent)
        .toContain("Wrong password or invalid");
    expect(await listVaults()).toEqual([]);
    input("backup-password", password);
    await click("Validate backup");
    await expect
        .poll(() => target.textContent)
        .toContain("1 encrypted records");
    expect(target.textContent).toContain(vaultId);
    expect(target.textContent).toContain("never overwrites");
    expect(target.querySelector("#backup-password")).toBeNull();
    expect(await listVaults()).toEqual([]);
    await click("Restore vault");
    await expect.poll(() => target.textContent).toContain("Restore verified");
    await click(name);
    input("password", password);
    await click("Unlock");
    await expect.poll(() => target.textContent).toContain(logical.name);
    await click("Lock vault");
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
    const reopened = await unlockVault(password, vaultId);
    expect(
        JSON.parse(
            await reopened.decryptSecret((await reopened.loadSecrets())[0]),
        ),
    ).toEqual(logical);
    reopened.close();
}, 20000);

it("shows identity collisions and cancellation without changing an existing vault", async () => {
    const name = `test-collision-ui-${crypto.randomUUID()}`;
    const source = await createVault("synthetic-backup-password", name);
    const id = source.id;
    const content = await source.exportBackup();
    source.close();
    const before = await listVaults();
    await start();
    await click("Restore encrypted backup");
    file(content);
    input("backup-password", "synthetic-backup-password");
    await click("Validate backup");
    await expect
        .poll(() => target.textContent)
        .toContain("This vault ID already exists");
    expect(target.textContent).toContain(id);
    expect(button("Restore vault").disabled).toBe(true);
    await click("Cancel restore");
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
    expect(target.textContent).not.toContain("Restore verified");
    expect(await listVaults()).toEqual(before);
    await click(name);
    input("password", "synthetic-backup-password");
    await click("Unlock");
    await expect.poll(() => target.textContent).toContain("Lock vault");
    await click("Lock vault");
}, 15000);

it("shows validation and storage failure without success or a partial destination, then restores an empty vault on retry", async () => {
    const source = await createVault(
        "synthetic-backup-password",
        `test-failure-ui-${crypto.randomUUID()}`,
    );
    const content = await source.exportBackup();
    source.close();
    for (const { name } of await indexedDB.databases()) {
        if (!name) continue;
        await new Promise<void>((resolve, reject) => {
            const request = indexedDB.deleteDatabase(name);
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
            request.onblocked = () => reject(new Error("Open test database"));
        });
    }
    await start();
    await click("Restore encrypted backup");
    file('{"secret":"PRIVATE-REJECTED-INPUT"}');
    input("backup-password", "synthetic-backup-password");
    await click("Validate backup");
    await expect
        .poll(() => target.textContent)
        .toContain("Wrong password or invalid");
    expect(target.textContent).not.toContain("PRIVATE-REJECTED-INPUT");
    expect(await listVaults()).toEqual([]);
    file(content);
    await click("Validate backup");
    await expect
        .poll(() => target.textContent)
        .toContain("0 encrypted records");
    const put = IDBObjectStore.prototype.put;
    const fault = vi
        .spyOn(IDBObjectStore.prototype, "put")
        .mockImplementation(function (this: IDBObjectStore, value, key) {
            if (this.name === "records")
                throw new DOMException(
                    "Synthetic quota error",
                    "QuotaExceededError",
                );
            return put.call(this, value, key);
        });
    try {
        await click("Restore vault");
        await expect
            .poll(() => target.textContent)
            .toContain("No destination was published");
    } finally {
        fault.mockRestore();
    }
    expect(target.textContent).not.toContain("Restore verified");
    expect(await listVaults()).toEqual([]);
    await expect.poll(() => button("Restore vault").disabled).toBe(false);
    await click("Restore vault");
    await expect.poll(() => target.textContent).toContain("Restore verified");
    expect(await listVaults()).toHaveLength(1);
}, 15000);
