import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import type { Secret, SshKey } from "$lib/secrets/secret";
import { prepareBackupRestore } from "$lib/vault/backup";
import { createVault, unlockVault, VaultHandle } from "$lib/vault/vault";
import Page from "./+page.svelte";

const password = "synthetic-ssh-master";
const date = "2026-01-02T03:04:05.000Z";
// Deliberately nonfunctional: storage must not parse, normalize, or use this key.
const privateKey =
    "  -----BEGIN OPENSSH PRIVATE KEY-----\nPRIVATE-SYNTHETIC 秘密\nnot-a-key\n-----END OPENSSH PRIVATE KEY-----\n  ";
const key: SshKey = {
    kind: "sshKey",
    id: "12345678-1234-4234-8234-123456789abc",
    name: "Synthetic SSH Key",
    privateKey,
    publicKey: "ssh-ed25519 SYNTHETIC-only comment 秘密\n",
    keyFingerprint: "SHA256:synthetic-stored-not-derived",
    notes: "SSH notes\nUnicode 秘密",
    favorite: true,
    fields: [{ name: "PIN", type: "hidden", value: "PRIVATE-FIELD" }],
    history: [{ secret: "PRIVATE-HISTORY", changedAt: date }],
    folderId: "12345678-1234-4234-8234-123456789abb",
    importSource: { format: "personal-json-v1", id: "synthetic-ssh-source" },
    createdAt: date,
    updatedAt: date,
};
const neighbors: Secret[] = [
    {
        kind: "login",
        name: "Neighbor Login",
        secret: "PRIVATE-LOGIN",
        uris: [],
    },
    {
        kind: "secureNote",
        name: "Neighbor Note",
        type: 0,
        notes: "PRIVATE-NOTE",
    },
    { kind: "card", name: "Neighbor Card", number: "001 002" },
    { kind: "identity", name: "Neighbor Identity", passportNumber: "00123" },
].map(
    (value, i) =>
        ({
            ...value,
            id: `12345678-1234-4234-8234-123456789ab${i}`,
            favorite: false,
            fields: [],
            history: [],
            createdAt: date,
            updatedAt: date,
        }) as Secret,
);
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
    await expect.poll(() => button("Neighbor Login").disabled).toBe(false);
}
async function start(original: SshKey = key) {
    const name = `SSH ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    await vault.mutateItems({
        folders: [{ id: key.folderId!, name: "SSH folder" }],
        upsert: [original, ...neighbors],
    });
    const id = vault.id;
    vault.close();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await unlock(name);
    return { name, id };
}
async function persisted(id: string) {
    const vault = await unlockVault(password, id);
    try {
        return await vault.readItems();
    } finally {
        vault.close();
    }
}
async function saved() {
    await expect.poll(() => target.textContent).toContain("Saved.");
    expect(target.querySelector("form")).toBeNull();
}
afterEach(async () => {
    vi.restoreAllMocks();
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
});

it("creates an SSH Key without Login fields, preserves multiline text, and reopens with masked details", async () => {
    const { id, name } = await start();
    await click("New SSH key");
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.querySelector("#username")).toBeNull();
    fill("name", "Created SSH Key");
    await click("Show Private key");
    fill("private-key", privateKey);
    fill("public-key", key.publicKey!);
    fill("key-fingerprint", key.keyFingerprint!);
    await click("Save");
    await saved();
    const created = (await persisted(id)).items.find(
        (item) => item.name === "Created SSH Key",
    )!;
    expect(created).toMatchObject({
        kind: "sshKey",
        privateKey,
        publicKey: key.publicKey,
        keyFingerprint: key.keyFingerprint,
        history: [],
    });
    expect(created).not.toHaveProperty("secret");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Show Private key");
    expect(target.textContent).toContain(privateKey);
    await click("Close details");
    await click("Created SSH Key");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Lock vault");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await unlock(name);
    await click("Created SSH Key");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Show Private key");
    expect(target.textContent).toContain(privateKey);
}, 25000);

it("cancels isolated edits, copies explicitly, saves only the selected key, and restores an exact mixed-type backup in clean storage", async () => {
    const { id } = await start();
    const original = await persisted(id);
    await click("★ Synthetic SSH Key");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    const copy = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
    await click("Copy Private key");
    await expect.poll(() => target.textContent).toContain("Copied.");
    expect(copy).toHaveBeenCalledWith(privateKey);
    copy.mockRejectedValueOnce(new Error("PRIVATE-exception"));
    await click("Copy Private key");
    await expect.poll(() => target.textContent).toContain("Copy failed.");
    expect(target.innerHTML).not.toContain("PRIVATE-exception");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Edit");
    expect(document.activeElement).toBe(control("name"));
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Show Private key");
    fill("private-key", "PRIVATE-abandoned");
    fill("public-key", "abandoned public");
    fill("key-fingerprint", "abandoned fingerprint");
    fill("field-value-0", "abandoned field");
    fill("notes", "abandoned notes");
    await click("Cancel");
    expect(target.innerHTML).not.toContain("PRIVATE-");
    expect(await persisted(id)).toEqual(original);
    await click("Edit");
    await click("Show Private key");
    expect(control("private-key").value).toBe(privateKey);
    fill("private-key", privateKey + "\nSaved edit");
    fill("public-key", key.publicKey + "saved public");
    fill("key-fingerprint", "stored-fingerprint-not-derived");
    await click("Save");
    await saved();
    const afterKeySave = await persisted(id);
    for (const item of neighbors)
        expect(afterKeySave.items).toContainEqual(item);
    for (const item of neighbors) {
        await click(item.name);
        await click("Edit");
        expect(control("name").value).toBe(item.name);
        fill("notes", "Discarded mixed edit");
        await click("Cancel");
        expect((await persisted(id)).items).toContainEqual(item);
        await click("Edit");
        if (item.kind !== "login") {
            expect(target.querySelector("#secret")).toBeNull();
            await click("Password generator");
            await click("Generate");
            expect(target.textContent).not.toContain("Use in Login draft");
            await click("Password generator");
        }
        fill("notes", `Mixed saved ${item.kind} 秘密\nSecond line`);
        await click("Save");
        await saved();
    }
    const expected = await persisted(id);
    expect(expected.items.find((item) => item.id === key.id)).toEqual({
        ...key,
        privateKey: privateKey + "\nSaved edit",
        publicKey: key.publicKey + "saved public",
        keyFingerprint: "stored-fingerprint-not-derived",
        updatedAt: expect.any(String),
    });
    for (const item of neighbors)
        expect(expected.items).toContainEqual({
            ...item,
            notes: `Mixed saved ${item.kind} 秘密\nSecond line`,
            updatedAt: expect.any(String),
        });
    expect(target.innerHTML).not.toContain("PRIVATE-");
    await click("Lock vault");
    const source = await unlockVault(password, id);
    await source.saveLockSettings({ timeoutMinutes: 15 });
    const backup = await source.exportBackup();
    const encrypted = await source.loadSecrets();
    source.close();
    expect(JSON.stringify(encrypted)).not.toContain("PRIVATE-");
    expect(backup).not.toContain("PRIVATE-");
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
    const restored = await unlockVault(password, id);
    try {
        expect(await restored.loadSecrets()).toEqual(encrypted);
        expect(await restored.loadLockSettings()).toEqual({
            timeoutMinutes: 15,
        });
    } finally {
        restored.close();
    }
}, 25000);

it.each([undefined, null, ""])(
    "preserves unchanged optional public key and fingerprint (%s), CRLF key text, and metadata",
    async (value) => {
        const original = {
            ...key,
            privateKey: privateKey.replaceAll("\n", "\r\n"),
            publicKey: value,
            keyFingerprint: value,
        };
        const { id } = await start(original);
        await click("★ Synthetic SSH Key");
        await click("Edit");
        fill("name", "Renamed SSH Key");
        await click("Save");
        await saved();
        expect(
            (await persisted(id)).items.find((item) => item.id === key.id),
        ).toEqual({
            ...original,
            name: "Renamed SSH Key",
            updatedAt: expect.any(String),
        });
    },
    15000,
);

it("rejects blank names and private keys without a write or key-bearing errors, but stores arbitrary nonblank key text", async () => {
    const { id } = await start();
    const original = await persisted(id);
    await click("New SSH key");
    fill("name", "  ");
    await click("Show Private key");
    fill("private-key", privateKey);
    await click("Save");
    expect(target.textContent).toContain("Invalid SSH Key");
    expect(target.querySelector('[role="alert"]')?.textContent).not.toContain(
        "PRIVATE-",
    );
    fill("name", "New synthetic key");
    fill("private-key", " \n\t ");
    await click("Save");
    expect(target.textContent).toContain("Invalid SSH Key");
    expect(await persisted(id)).toEqual(original);
    fill("private-key", "PRIVATE-not-a-parsed-key");
    await click("Save");
    await saved();
    expect((await persisted(id)).items).toContainEqual(
        expect.objectContaining({
            name: "New synthetic key",
            privateKey: "PRIVATE-not-a-parsed-key",
        }),
    );
    const vault = await unlockVault(password, id);
    try {
        const before = await vault.readItems();
        for (const bad of [undefined, null, 123, "", " \n\t"]) {
            await expect(
                vault.mutateItems({
                    upsert: [{ ...key, privateKey: bad } as unknown as Secret],
                }),
            ).rejects.toThrow();
            expect(await vault.readItems()).toEqual(before);
        }
        for (const bad of [
            { publicKey: 42 },
            { keyFingerprint: false },
            { privateKey: "PRIVATE-unknown", extra: "PRIVATE-value" },
        ]) {
            await expect(
                vault.mutateItems({
                    upsert: [{ ...key, ...bad } as unknown as Secret],
                }),
            ).rejects.toThrow(
                "Malformed vault snapshot. No partial records or folders were loaded. Check the complete backup; do not clear browser data.",
            );
            expect(await vault.readItems()).toEqual(before);
        }
    } finally {
        vault.close();
    }
}, 25000);
it.each([
    "-----BEGIN RSA PRIVATE KEY-----\nPRIVATE-SYNTHETIC-PEM\n-----END RSA PRIVATE KEY-----\n",
    "-----BEGIN ENCRYPTED PRIVATE KEY-----\nPRIVATE-SYNTHETIC-ENCRYPTED\n-----END ENCRYPTED PRIVATE KEY-----\n",
    "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: synthetic\nPRIVATE-SYNTHETIC-PPK\n",
])(
    "stores opaque key representations unchanged through edit and reopen",
    async (privateKey) => {
        const original = { ...key, privateKey };
        const { id } = await start(original);
        await click("★ Synthetic SSH Key");
        await click("Edit");
        fill("name", "Renamed representation");
        await click("Save");
        await saved();
        expect(
            (await persisted(id)).items.find((item) => item.id === key.id),
        ).toEqual({
            ...original,
            name: "Renamed representation",
            updatedAt: expect.any(String),
        });
    },
    15000,
);

it.each(["encryption", "readback"] as const)(
    "does not claim an SSH Key save after %s failure and recovers the authoritative record",
    async (operation) => {
        const { id } = await start();
        await click("★ Synthetic SSH Key");
        await click("Edit");
        await click("Show Private key");
        fill("private-key", "PRIVATE-CHANGED");
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
        await expect
            .poll(() => button("★ Synthetic SSH Key").disabled)
            .toBe(false);
        await click("★ Synthetic SSH Key");
        expect(target.innerHTML).not.toContain("PRIVATE-");
        expect(
            (await persisted(id)).items.find((record) => record.id === key.id),
        ).toEqual(
            operation === "encryption"
                ? key
                : {
                      ...key,
                      privateKey: "PRIVATE-CHANGED",
                      updatedAt: expect.any(String),
                  },
        );
        expect(
            (await persisted(id)).items.find(
                (record) => record.id === neighbors[0].id,
            ),
        ).toEqual(neighbors[0]);
    },
    15000,
);

it.each(["draft", "copy", "read", "write", "acknowledgement"] as const)(
    "locks an SSH Key during %s and rejects late UI results",
    async (operation) => {
        const { id, name } = await start();
        await click("★ Synthetic SSH Key");
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
            await click("Show Private key");
            await click("Copy Private key");
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
            await click("Show Private key");
            fill("private-key", "PRIVATE-LATE");
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
        await click("★ Synthetic SSH Key");
        expect(target.innerHTML).not.toContain("PRIVATE-");
        expect(
            (await persisted(id)).items.find((record) => record.id === key.id),
        ).toEqual(
            operation === "acknowledgement"
                ? {
                      ...key,
                      privateKey: "PRIVATE-LATE",
                      updatedAt: expect.any(String),
                  }
                : key,
        );
        expect(
            (await persisted(id)).items.find(
                (record) => record.id === neighbors[0].id,
            ),
        ).toEqual(neighbors[0]);
    },
    15000,
);
