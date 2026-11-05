import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import { createVault, unlockVault } from "$lib/vault/vault";
import Page from "./+page.svelte";

const password = "synthetic-lock-master";
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
    button(name).click();
    flushSync();
    await tick();
}
function fill(id: string, value: string) {
    const input = target.querySelector<HTMLInputElement>(`#${id}`)!;
    input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
}
async function start() {
    const name = `Lock ${crypto.randomUUID()}`;
    const vault = await createVault(password, name);
    await vault.saveLockSettings({ timeoutMinutes: 1 });
    const id = vault.id;
    vault.close();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await expect
        .poll(() => target.textContent, { timeout: 10000 })
        .toContain(name);
    await click(name);
    fill("password", password);
    await click("Unlock");
    await expect
        .poll(() => button("New secret").disabled, { timeout: 10000 })
        .toBe(false);
    return { id, name };
}
async function locked() {
    await expect
        .poll(() => target.textContent, { timeout: 10000 })
        .toContain("Vault locked.");
    expect(target.textContent).not.toContain("New secret");
    expect(target.querySelector("#notes")).toBeNull();
    expect(target.innerHTML).not.toContain("PRIVATE-DRAFT");
}
afterEach(async () => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
});

it("saves a labeled per-vault timeout and reloads it after lock and authentication", async () => {
    const { id, name } = await start();
    const select = target.querySelector<HTMLSelectElement>("#lock-timeout")!;
    expect(select.value).toBe("1");
    select.value = "15";
    select.dispatchEvent(new Event("change", { bubbles: true }));
    await click("Save lock settings");
    await expect
        .poll(() => target.textContent, { timeout: 10000 })
        .toContain("Lock settings saved.");
    const reopened = await unlockVault(password, id);
    expect(await reopened.loadLockSettings()).toEqual({ timeoutMinutes: 15 });
    reopened.close();
    await click("Lock vault");
    await locked();
    await click(name);
    fill("password", password);
    await click("Unlock");
    await expect
        .poll(
            () =>
                target.querySelector<HTMLSelectElement>("#lock-timeout")?.value,
        )
        .toBe("15");
}, 30000);

it("locks an active draft through the visible inactivity timer and discards it", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    await start();
    await click("New secret");
    fill("name", "PRIVATE-DRAFT");
    const now = Date.now();
    vi.spyOn(Date, "now").mockReturnValue(now + 60_001);
    await vi.advanceTimersByTimeAsync(1000);
    flushSync();
    await locked();
}, 30000);

it.each(["focus", "pageshow", "visibilitychange", "keydown", "click"])(
    "checks the expired deadline before %s can revive the session",
    async (event) => {
        await start();
        await click("New secret");
        fill("name", "PRIVATE-DRAFT");
        vi.spyOn(Date, "now").mockReturnValue(Date.now() + 60_001);
        if (event === "visibilitychange")
            document.dispatchEvent(new Event(event));
        else if (event === "click") button("Save").click();
        else
            window.dispatchEvent(
                new Event(event, { cancelable: true, bubbles: true }),
            );
        flushSync();
        await locked();
        expect(target.textContent).not.toContain("Saved.");
    },
    30000,
);

it("extends an unexpired deadline on activity, not on resume alone", async () => {
    await start();
    const now = Date.now();
    const clock = vi.spyOn(Date, "now").mockReturnValue(now + 30_000);
    window.dispatchEvent(new Event("keydown"));
    clock.mockReturnValue(now + 65_000);
    window.dispatchEvent(new Event("focus"));
    flushSync();
    expect(target.textContent).toContain("New secret");
    clock.mockReturnValue(now + 90_001);
    window.dispatchEvent(new Event("focus"));
    flushSync();
    await locked();
}, 30000);

it.each(["settings", "backup"] as const)(
    "does not publish a late %s result after automatic lock",
    async (operation) => {
        const { id } = await start();
        let release!: () => void;
        let sent!: () => void;
        const gate = new Promise<void>((resolve) => {
            release = resolve;
        });
        const started = new Promise<void>((resolve) => {
            sent = resolve;
        });
        const encrypt = crypto.subtle.encrypt.bind(crypto.subtle);
        vi.spyOn(crypto.subtle, "encrypt").mockImplementationOnce(
            async (...args) => {
                sent();
                await gate;
                return encrypt(...args);
            },
        );
        const download = vi.spyOn(URL, "createObjectURL");
        if (operation === "settings") {
            const select =
                target.querySelector<HTMLSelectElement>("#lock-timeout")!;
            select.value = "30";
            select.dispatchEvent(new Event("change", { bubbles: true }));
            await click("Save lock settings");
        } else await click("Download encrypted backup");
        await started;
        vi.spyOn(Date, "now").mockReturnValue(Date.now() + 60_001);
        window.dispatchEvent(new Event("focus"));
        flushSync();
        await locked();
        release();
        await new Promise((resolve) => setTimeout(resolve, 100));
        flushSync();
        expect(target.textContent).not.toContain("Lock settings saved.");
        expect(target.textContent).not.toContain("Download requested.");
        expect(download).not.toHaveBeenCalled();
        vi.restoreAllMocks();
        const reopened = await unlockVault(password, id);
        expect(await reopened.loadLockSettings()).toEqual({
            timeoutMinutes: 1,
        });
        reopened.close();
    },
    30000,
);

it("checks the deadline before downloading a backup when the timer is suspended", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    await start();
    let release!: () => void;
    let sent!: () => void;
    const gate = new Promise<void>((resolve) => {
        release = resolve;
    });
    const started = new Promise<void>((resolve) => {
        sent = resolve;
    });
    const encrypt = crypto.subtle.encrypt.bind(crypto.subtle);
    vi.spyOn(crypto.subtle, "encrypt").mockImplementationOnce(
        async (...args) => {
            sent();
            await gate;
            return encrypt(...args);
        },
    );
    const download = vi.spyOn(URL, "createObjectURL");
    await click("Download encrypted backup");
    await started;
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 60_001);
    release();
    await locked();
    expect(download).not.toHaveBeenCalled();
    expect(target.textContent).not.toContain("Download requested.");
}, 30000);

it("keeps settings and data isolated between vaults and requires authentication after page restart", async () => {
    const { name } = await start();
    if (component) await unmount(component);
    component = mount(Page, { target });
    flushSync();
    await expect
        .poll(() => target.textContent, { timeout: 10000 })
        .toContain("Select a vault to unlock");
    expect(target.querySelector("#lock-timeout")).toBeNull();
    await click(name);
    fill("password", "wrong-password");
    await click("Unlock");
    await expect
        .poll(() => target.textContent, { timeout: 10000 })
        .toContain("Wrong password");
    expect(target.querySelector("#lock-timeout")).toBeNull();
    const second = await createVault(password, `Other ${crypto.randomUUID()}`);
    expect(await second.loadLockSettings()).toEqual({ timeoutMinutes: 5 });
    second.close();
}, 30000);
