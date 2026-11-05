import { flushSync, mount, tick, unmount } from "svelte";
import * as v from "valibot";
import { afterEach, expect, it, vi } from "vitest";
import { preparePersonalJsonImport } from "$lib/import/import";
import { SecretSchema } from "$lib/secrets/secret";
import { createVault, unlockVault, VaultHandle } from "$lib/vault/vault";
import expected from "../../../../docs/fixtures/password-manager/export-expected.json";
import local from "../../../../docs/fixtures/password-manager/export-local.json";
import Page from "./+page.svelte";

let target: HTMLDivElement;
let component: ReturnType<typeof mount>;
const password = "synthetic-export-password";
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
async function start(items: unknown[] = local.items, folders = local.folders) {
    const name = `test-json-export-${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    await vault.mutateItems({
        upsert: v.parse(v.array(SecretSchema), items),
        folders,
    });
    const before = await vault.readItems();
    const id = vault.id;
    vault.close();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
    await click(name);
    const input = target.querySelector<HTMLInputElement>("#password");
    if (!input) throw new Error("Missing master password control");
    input.value = password;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await click("Unlock");
    await expect.poll(() => target.textContent).toContain("New secret");
    await expect.poll(() => button("New secret").disabled).toBe(false);
    return { id, before };
}
afterEach(async () => {
    vi.restoreAllMocks();
    vi.useRealTimers();
    if (component) await unmount(component);
    target?.remove();
});

it("exports all five types against independent reference JSON only after warning and confirmation", async () => {
    const { id, before } = await start();
    let download: Blob | undefined;
    vi.spyOn(URL, "createObjectURL").mockImplementation((blob) => {
        download = blob as Blob;
        return "blob:synthetic-export";
    });
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    await click("Export Bitwarden JSON");
    await expect.poll(() => target.textContent).toContain("5 active items");
    expect(target.textContent).toContain("2 folders");
    expect(target.textContent).toContain("not encrypted");
    expect(target.textContent).toContain("0 Trash items excluded");
    expect(target.textContent).toContain("not a recovery backup");
    expect(target.textContent).not.toContain("SYNTHETIC-NOT-A-CREDENTIAL");
    expect(download).toBeUndefined();
    await click("Download plaintext JSON");
    await expect.poll(() => download).toBeDefined();
    if (!download) throw new Error("Missing plaintext download");
    expect(JSON.parse(await download.text())).toEqual(expected);
    await click("Lock vault");
    const reopened = await unlockVault(password, id);
    expect(await reopened.readItems()).toEqual(before);
    reopened.close();
}, 30000);

it("exports the independent five-type subset, excludes Trash, previews and atomically imports it with fresh identities, and rejects an unconfirmed repeat", async () => {
    const empty = {
        id: "10000000-0000-4000-8000-000000000003",
        name: "Empty portability folder 日本語",
    };
    const trash = {
        ...local.items[1],
        id: "20000000-0000-4000-8000-000000000006",
        name: "EXCLUDED-SYNTHETIC-TRASH",
        deletedAt: "2025-04-01T00:00:00.000Z",
    };
    const source = await start(
        [...local.items, trash],
        [...local.folders, empty],
    );
    let download: Blob | undefined;
    vi.spyOn(URL, "createObjectURL").mockImplementation((blob) => {
        download = blob as Blob;
        return "blob:synthetic-portability";
    });
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    await click("Export Bitwarden JSON");
    await expect
        .poll(() => target.textContent)
        .toContain("5 active items; 3 folders; 1 Trash items excluded");
    expect(target.textContent).toContain("not encrypted");
    expect(download).toBeUndefined();
    await click("Download plaintext JSON");
    if (!download) throw new Error("Missing confirmed synthetic export");
    const text = await download.text();
    expect(JSON.parse(text)).toEqual({
        ...expected,
        folders: [...expected.folders, empty],
    });
    expect(text).not.toContain(trash.name);
    const original = await unlockVault(password, source.id);
    expect(await original.readItems()).toEqual(source.before);
    original.close();
    await unmount(component);
    component = undefined as unknown as ReturnType<typeof mount>;
    target.remove();

    const destination = await start([], []);
    const mutate = vi.spyOn(VaultHandle.prototype, "mutateItems");
    const selectFile = () => {
        const input = target.querySelector<HTMLInputElement>("#import-file");
        if (!input) throw new Error("Missing import file control");
        const transfer = new DataTransfer();
        transfer.items.add(
            new File([text], "synthetic-export.json", {
                type: "application/json",
            }),
        );
        input.files = transfer.files;
        input.dispatchEvent(new Event("change", { bubbles: true }));
        flushSync();
    };
    await click("Import Bitwarden JSON");
    expect(target.textContent).toContain("Plaintext files expose credentials");
    selectFile();
    await click("Validate import");
    await expect.poll(() => target.textContent).toContain("Import preview");
    for (const label of [
        "Login: 1",
        "Secure Note: 1",
        "Card: 1",
        "Identity: 1",
        "SSH Key: 1",
        "Folders: 3",
        "Source-ID matches: 0",
        "Logical duplicates: 0",
    ])
        expect(target.textContent).toContain(label);
    expect(mutate).not.toHaveBeenCalled();
    const reader = await unlockVault(password, destination.id);
    expect(await reader.readItems()).toEqual(destination.before);
    await click("Import all as new");
    await expect.poll(() => target.textContent).toContain("Import verified");
    expect(mutate).toHaveBeenCalledTimes(1);
    expect(mutate.mock.calls[0][0].upsert).toHaveLength(5);
    expect(mutate.mock.calls[0][0].folders).toHaveLength(3);
    const imported = await reader.readItems();
    expect(imported.folders).toHaveLength(3);
    for (const folder of [...local.folders, empty]) {
        const actual = imported.folders.find(
            (value) => value.name === folder.name,
        );
        expect(actual).toEqual({ id: expect.any(String), name: folder.name });
        expect(actual?.id).not.toBe(folder.id);
    }
    expect(imported.items).toHaveLength(5);
    for (const item of v.parse(v.array(SecretSchema), local.items)) {
        const actual = imported.items.find((value) => value.name === item.name);
        const folder = local.folders.find(
            (value) => value.id === item.folderId,
        );
        expect(actual).toEqual({
            ...item,
            id: expect.any(String),
            folderId: folder
                ? imported.folders.find((value) => value.name === folder.name)
                      ?.id
                : null,
            importSource: { format: "personal-json-v1", id: item.id },
            notes: item.notes ?? null,
            createdAt: "2025-01-01T00:00:00.000Z",
            updatedAt: "2025-03-01T00:00:00.000Z",
            history:
                item.kind === "login"
                    ? [
                          {
                              secret: "SYNTHETIC-PRIOR-ONE",
                              changedAt: "2025-01-02T03:04:05.000Z",
                          },
                          { secret: "", changedAt: "2025-02-03T04:05:06.000Z" },
                      ]
                    : [],
        });
        expect(actual?.id).not.toBe(item.id);
        expect(actual?.deletedAt).toBeUndefined();
    }
    const repeat = await preparePersonalJsonImport(reader, text);
    expect(repeat.issues).toEqual([]);
    expect(repeat.duplicates).toEqual({ sourceIds: 5, logical: 5 });
    await expect(repeat.commit()).rejects.toThrow("Duplicates found");
    repeat.retire();
    expect(mutate).toHaveBeenCalledTimes(1);
    expect(await reader.readItems()).toEqual(imported);
    reader.close();

    await click("Export Bitwarden JSON");
    await expect.poll(() => target.textContent).toContain("5 active items");
    await click("Import Bitwarden JSON");
    selectFile();
    await click("Validate import");
    await expect
        .poll(() => target.textContent)
        .toContain("Source-ID matches: 5");
    expect(button("Import all as new").disabled).toBe(true);
    await click("Lock vault");
    expect(target.textContent).not.toContain("Download plaintext JSON");
    expect(target.textContent).not.toContain("Import preview");
    const reopened = await unlockVault(password, destination.id);
    expect(await reopened.readItems()).toEqual(imported);
    reopened.close();
}, 30000);

it.each(["cancel", "lock", "timeout", "unmount", "read-error"])(
    "discards a late plaintext preview after %s",
    async (operation) => {
        await start();
        vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
        let release!: () => void;
        let sent!: () => void;
        const gate = new Promise<void>((resolve) => {
            release = resolve;
        });
        const started = new Promise<void>((resolve) => {
            sent = resolve;
        });
        const decrypt = crypto.subtle.decrypt.bind(crypto.subtle);
        vi.spyOn(crypto.subtle, "decrypt").mockImplementationOnce(
            async (...args) => {
                sent();
                await gate;
                if (operation === "read-error")
                    throw new Error("SYNTHETIC-PRIVATE-ERROR");
                return decrypt(...args);
            },
        );
        const download = vi.spyOn(URL, "createObjectURL");
        await click("Export Bitwarden JSON");
        await started;
        expect(button("Download plaintext JSON").disabled).toBe(true);
        if (operation === "cancel") await click("Cancel export");
        else if (operation === "lock") await click("Lock vault");
        else if (operation === "timeout")
            vi.spyOn(Date, "now").mockReturnValue(Date.now() + 300_001);
        else if (operation === "unmount") {
            await unmount(component);
            component = undefined as unknown as ReturnType<typeof mount>;
        }
        release();
        if (operation === "timeout" || operation === "lock")
            await expect
                .poll(() => target.textContent)
                .toContain("Select a vault to unlock");
        else if (operation === "read-error")
            await expect
                .poll(() => target.textContent)
                .toContain("Export blocked");
        else await new Promise((resolve) => setTimeout(resolve, 150));
        flushSync();
        expect(target.textContent).not.toContain("5 active items");
        expect(target.textContent).not.toContain("SYNTHETIC-PRIVATE-ERROR");
        expect(target.textContent).not.toContain(
            "Plaintext download requested",
        );
        expect(download).not.toHaveBeenCalled();
    },
    30000,
);

it("blocks unrepresentable legacy Boolean data and identifies the field without its value", async () => {
    const items = structuredClone(local.items);
    const fields = items[0].fields;
    if (!fields) throw new Error("Missing synthetic custom fields");
    fields[2].value = "SYNTHETIC-PRIVATE-VALUE";
    await start(items);
    const download = vi.spyOn(URL, "createObjectURL");
    await click("Export Bitwarden JSON");
    await expect
        .poll(() => target.textContent)
        .toContain("/items/0/fields/2/value");
    expect(button("Download plaintext JSON").disabled).toBe(true);
    expect(target.textContent).not.toContain("SYNTHETIC-PRIVATE-VALUE");
    expect(download).not.toHaveBeenCalled();
    await click("Cancel export");
}, 30000);

it("locks before an expired confirmation and never downloads plaintext", async () => {
    await start();
    await click("Export Bitwarden JSON");
    await expect.poll(() => target.textContent).toContain("5 active items");
    const download = vi.spyOn(URL, "createObjectURL");
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 300_001);
    button("Download plaintext JSON").click();
    flushSync();
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
    expect(download).not.toHaveBeenCalled();
}, 30000);

it("cancels before generating plaintext and returns focus, without changing the vault", async () => {
    const { id, before } = await start();
    const download = vi.spyOn(URL, "createObjectURL");
    await click("Export Bitwarden JSON");
    await expect.poll(() => target.textContent).toContain("5 active items");
    expect(document.activeElement).toBe(button("Cancel export"));
    await click("Cancel export");
    expect(document.activeElement).toBe(button("Export Bitwarden JSON"));
    expect(target.textContent).not.toContain("Plaintext credential export");
    expect(download).not.toHaveBeenCalled();
    await click("Lock vault");
    expect(target.textContent).not.toContain("Export Bitwarden JSON");
    const reopened = await unlockVault(password, id);
    expect(await reopened.readItems()).toEqual(before);
    reopened.close();
}, 30000);

it("explicitly excludes Trash but includes every folder, including empty and duplicate-named folders", async () => {
    const items = local.items.map((item) => ({
        ...item,
        deletedAt: "2025-04-01T00:00:00.000Z",
    }));
    await start(items, [
        ...local.folders,
        { id: "10000000-0000-4000-8000-000000000003", name: "Synthetic" },
    ]);
    let download: Blob | undefined;
    vi.spyOn(URL, "createObjectURL").mockImplementation((blob) => {
        download = blob as Blob;
        return "blob:trash-export";
    });
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    await click("Export Bitwarden JSON");
    await expect
        .poll(() => target.textContent)
        .toContain("0 active items; 3 folders; 5 Trash items excluded");
    await click("Download plaintext JSON");
    if (!download) throw new Error("Missing plaintext download");
    expect(JSON.parse(await download.text())).toEqual({
        encrypted: false,
        items: [],
        folders: [
            ...expected.folders,
            { id: "10000000-0000-4000-8000-000000000003", name: "Synthetic" },
        ],
    });
}, 30000);

it.each(["serialize", "url", "click"])(
    "reports a %s download failure without leaking exception contents and permits retry",
    async (operation) => {
        await start();
        const revoke = vi.spyOn(URL, "revokeObjectURL");
        const url = vi
            .spyOn(URL, "createObjectURL")
            .mockReturnValue("blob:failed-export");
        const anchor = vi
            .spyOn(HTMLAnchorElement.prototype, "click")
            .mockImplementation(() => {});
        await click("Export Bitwarden JSON");
        await expect.poll(() => target.textContent).toContain("5 active items");
        const failure = () => {
            throw new Error("SYNTHETIC-PRIVATE-ERROR");
        };
        if (operation === "serialize") {
            const stringify = JSON.stringify;
            let fail = true;
            vi.spyOn(JSON, "stringify").mockImplementation(
                (...args: Parameters<typeof JSON.stringify>) => {
                    const data = args[0];
                    if (
                        fail &&
                        data &&
                        typeof data === "object" &&
                        "encrypted" in data &&
                        data.encrypted === false
                    ) {
                        fail = false;
                        return failure();
                    }
                    return stringify(...args);
                },
            );
        } else if (operation === "url") url.mockImplementationOnce(failure);
        else anchor.mockImplementationOnce(failure);
        await click("Download plaintext JSON");
        expect(target.textContent).toContain("Plaintext export failed");
        expect(target.textContent).not.toContain("SYNTHETIC-PRIVATE-ERROR");
        expect(target.textContent).not.toContain("download requested");
        expect(
            document.querySelector('a[download$="-plaintext.json"]'),
        ).toBeNull();
        if (operation === "click")
            expect(revoke).toHaveBeenCalledWith("blob:failed-export");
        await click("Download plaintext JSON");
        expect(target.textContent).toContain("Plaintext download requested");
        await click("Lock vault");
        expect(revoke).toHaveBeenCalledWith("blob:failed-export");
    },
    30000,
);
