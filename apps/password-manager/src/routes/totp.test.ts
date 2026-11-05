import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import { createVault, unlockVault } from "$lib/vault/vault";
import { prepareBackupRestore } from "$lib/vault/backup";
import type { Login } from "$lib/secrets/secret";
import Page from "./+page.svelte";

const seed = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
const password = "synthetic-totp-master";
let component: ReturnType<typeof mount> | undefined;
let target: HTMLDivElement;
function button(name: string) {
    const result = [...target.querySelectorAll("button")].find(
        (button) =>
            (
                button.querySelector(".vault-name")?.textContent ??
                button.textContent
            )?.trim() === name,
    );
    if (!result) throw new Error(`Missing button: ${name}`);
    return result;
}
async function click(name: string) {
    expect(button(name).disabled).toBe(false);
    button(name).click();
    flushSync();
    await tick();
}
function fill(id: string, value: string) {
    const input = target.querySelector<HTMLInputElement>(`#${id}`);
    if (!input) throw new Error(`Missing input: ${id}`);
    input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
}
async function start(totp = seed) {
    const name = `TOTP ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    const record: Login = {
        kind: "login",
        id: crypto.randomUUID(),
        name: "Synthetic TOTP Login",
        totp,
        favorite: false,
        uris: [],
        fields: [],
        history: [],
        createdAt: "2026-01-01T00:00:00.000Z",
        updatedAt: "2026-01-01T00:00:00.000Z",
    };
    await vault.prepareItems();
    await vault.encryptAndStore(JSON.stringify(record), record.id);
    const encrypted = await vault.loadSecrets();
    const id = vault.id;
    vault.close();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await expect.poll(() => target.textContent).toContain(name);
    await click(name);
    fill("password", password);
    await click("Unlock");
    await expect.poll(() => button(record.name).disabled).toBe(false);
    return { record, id, name, encrypted };
}
afterEach(async () => {
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
    vi.restoreAllMocks();
});

it("shows local codes, reconciles a time boundary and tab resume, and reports copy failure without exposing the key", async () => {
    const { record } = await start();
    const now = vi.spyOn(Date, "now").mockReturnValue(29999);
    await click(record.name);
    await expect.poll(() => target.textContent).toContain("755224");
    expect(target.textContent).toContain("1 second remaining");
    expect(target.innerHTML).not.toContain(seed);
    now.mockReturnValue(30000);
    await expect.poll(() => target.textContent).toContain("287082");
    expect(target.textContent).toContain("30 seconds remaining");
    const write = vi
        .spyOn(navigator.clipboard, "writeText")
        .mockRejectedValue(new Error(seed));
    await click("Copy TOTP code");
    await expect
        .poll(() => target.querySelector('[role="alert"]')?.textContent)
        .toContain("Copy failed");
    expect(write).toHaveBeenCalledWith("287082");
    expect(target.innerHTML).not.toContain(seed);
    now.mockReturnValue(90000);
    document.dispatchEvent(new Event("visibilitychange"));
    flushSync();
    expect(target.textContent).not.toContain("287082");
    await expect.poll(() => target.textContent).toContain("969429");
    expect(target.textContent).toContain("30 seconds remaining");
    write.mockResolvedValue();
    await click("Copy TOTP code");
    await expect.poll(() => target.textContent).toContain("Copied.");
    expect(write).toHaveBeenLastCalledWith("969429");
}, 15000);

it("does not copy an expired code before the next timer or resume event", async () => {
    const { record } = await start();
    const now = vi.spyOn(Date, "now").mockReturnValue(29999);
    await click(record.name);
    await expect.poll(() => target.textContent).toContain("755224");
    const write = vi
        .spyOn(navigator.clipboard, "writeText")
        .mockResolvedValue();
    now.mockReturnValue(30000);
    await click("Copy TOTP code");
    expect(write).not.toHaveBeenCalledWith("755224");
    await expect.poll(() => target.textContent).toContain("287082");
}, 15000);

it("hides codes while suspended and reconciles on pageshow and focus", async () => {
    const { record } = await start();
    const now = vi.spyOn(Date, "now").mockReturnValue(0);
    await click(record.name);
    await expect.poll(() => target.textContent).toContain("755224");
    const visibility = vi
        .spyOn(document, "visibilityState", "get")
        .mockReturnValue("hidden");
    document.dispatchEvent(new Event("visibilitychange"));
    flushSync();
    expect(target.textContent).not.toContain("755224");
    expect(button("Copy Authenticator key (TOTP)")).toBeDefined();
    now.mockReturnValue(60000);
    visibility.mockReturnValue("visible");
    window.dispatchEvent(new Event("pageshow"));
    await expect.poll(() => target.textContent).toContain("359152");
    now.mockReturnValue(90000);
    window.dispatchEvent(new Event("focus"));
    await expect.poll(() => target.textContent).toContain("969429");
}, 15000);

it("uses completion time for the countdown after slow local cryptography", async () => {
    const { record } = await start();
    const now = vi.spyOn(Date, "now").mockReturnValue(0);
    const sign = crypto.subtle.sign.bind(crypto.subtle);
    vi.spyOn(crypto.subtle, "sign").mockImplementationOnce(async (...args) => {
        const result = await sign(...args);
        now.mockReturnValue(5000);
        return result;
    });
    await click(record.name);
    await expect.poll(() => target.textContent).toContain("755224");
    expect(target.textContent).toContain("25 seconds remaining");
}, 15000);

it("reports local cryptography failure without exposing the key", async () => {
    const { record } = await start();
    vi.spyOn(crypto.subtle, "sign").mockRejectedValue(new Error(seed));
    await click(record.name);
    await expect
        .poll(() => target.querySelector('[role="alert"]')?.textContent)
        .toContain("TOTP generation failed");
    expect(target.innerHTML).not.toContain(seed);
    expect(target.textContent).not.toContain("Copy TOTP code");
}, 15000);

it.each([
    ["Close details", "sign"],
    ["Lock vault", "sign"],
    ["Close details", "copy"],
    ["Lock vault", "copy"],
] as const)(
    "disposes TOTP work on %s and ignores a late %s result",
    async (action, operation) => {
        const { record } = await start();
        const now = vi.spyOn(Date, "now").mockReturnValue(0);
        await click(record.name);
        await expect.poll(() => target.textContent).toContain("755224");
        let release!: () => void;
        const gate = new Promise<void>((resolve) => {
            release = resolve;
        });
        let started = false;
        const sign = crypto.subtle.sign.bind(crypto.subtle);
        const signing = vi.spyOn(crypto.subtle, "sign");
        if (operation === "sign") {
            signing.mockImplementationOnce(async (...args) => {
                started = true;
                await gate;
                return sign(...args);
            });
            now.mockReturnValue(30000);
            document.dispatchEvent(new Event("visibilitychange"));
        } else {
            vi.spyOn(navigator.clipboard, "writeText").mockImplementationOnce(
                async () => {
                    started = true;
                    await gate;
                },
            );
            await click("Copy TOTP code");
        }
        await expect.poll(() => started).toBe(true);
        await click(action);
        expect(target.textContent).not.toContain("755224");
        expect(target.textContent).not.toContain("TOTP code");
        expect(target.innerHTML).not.toContain(seed);
        const calls = signing.mock.calls.length;
        release();
        await new Promise((resolve) => setTimeout(resolve, 1100));
        flushSync();
        expect(signing.mock.calls.length).toBe(calls);
        expect(target.textContent).not.toContain("287082");
        expect(target.textContent).not.toContain("Copied.");
        expect(target.textContent).not.toContain("Copy failed");
    },
    15000,
);

it.each([
    `  ${seed.toLowerCase()}  `,
    `otpauth://totp/Synthetic%3Atest?issuer=Synthetic&secret=${seed}&algorithm=SHA256&digits=7&period=60`,
    "otpauth://hotp/Synthetic?secret=UNSUPPORTED-PRIVATE&counter=1",
])(
    "preserves exact stored key case %# through display, save/reopen and clean backup restore",
    async (totp) => {
        const { record, id, encrypted } = await start(totp);
        vi.spyOn(Date, "now").mockReturnValue(0);
        await click(record.name);
        if (totp.includes("hotp")) {
            await expect
                .poll(() => target.querySelector('[role="alert"]')?.textContent)
                .toContain("Only otpauth TOTP");
            expect(target.innerHTML).not.toContain("UNSUPPORTED-PRIVATE");
        } else {
            await expect
                .poll(() => target.textContent)
                .toContain("seconds remaining");
            expect(target.innerHTML).not.toContain(totp.trim());
        }
        const readOnly = await unlockVault(password, id);
        expect(await readOnly.loadSecrets()).toEqual(encrypted);
        expect(JSON.parse(await readOnly.decryptSecret(encrypted[0]))).toEqual(
            record,
        );
        readOnly.close();
        await click("Edit");
        fill("notes", "Saved without normalizing TOTP");
        await click("Save");
        await expect.poll(() => target.textContent).toContain("Saved.");
        await click("Lock vault");
        const reopened = await unlockVault(password, id);
        const saved = await reopened.loadSecrets();
        const logical = JSON.parse(await reopened.decryptSecret(saved[0]));
        expect(logical.totp).toBe(totp);
        expect(logical.notes).toBe("Saved without normalizing TOTP");
        const backup = await reopened.exportBackup();
        reopened.close();
        expect(backup).not.toContain(seed);
        expect(backup).not.toContain("UNSUPPORTED-PRIVATE");
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
                    reject(new Error("Test database still open"));
            });
        }
        await (await prepareBackupRestore(password, backup)).restore();
        const restored = await unlockVault(password, id);
        expect(await restored.loadSecrets()).toEqual(saved);
        expect(JSON.parse(await restored.decryptSecret(saved[0]))).toEqual(
            logical,
        );
        restored.close();
    },
    20000,
);
