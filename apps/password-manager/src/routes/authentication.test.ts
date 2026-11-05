import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it } from "vitest";
import {
    fixturePassword,
    legacyVault,
} from "$lib/vault/authentication-fixtures";
import { createVault } from "$lib/vault/vault";
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

function button(name: string): HTMLButtonElement {
    const element = [...target.querySelectorAll("button")].find(
        (button) => button.textContent?.trim() === name,
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

function input(id: string, value: string) {
    const element = target.querySelector<HTMLInputElement>(`#${id}`);
    if (!element) throw new Error(`Missing input: ${id}`);
    element.value = value;
    element.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
}

async function wrongPassword() {
    input("password", "wrong-password");
    await click("Unlock");
    await expect.poll(() => target.textContent).toContain("Wrong password");
    expect(target.textContent).not.toContain("Lock vault");
    expect(target.textContent).not.toContain(
        "Stored locally in your encrypted vault.",
    );
    expect(target.querySelector<HTMLInputElement>("#password")?.type).toBe(
        "password",
    );
}

async function correctPassword() {
    input("password", fixturePassword);
    await click("Unlock");
    await expect.poll(() => target.textContent).toContain("Lock vault");
    expect(target.querySelector("#password")).toBeNull();
    expect(target.textContent).not.toContain(fixturePassword);
    await click("Lock vault");
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
}

afterEach(async () => {
    if (component) await unmount(component);
    target?.remove();
});

it("creates an empty vault visibly and authenticates it after lock and remount", async () => {
    const name = `test-visible-new-${crypto.randomUUID()}`;
    await start();
    await click("Create new vault");
    input("vault-name", name);
    input("password", fixturePassword);
    input("confirm", fixturePassword);
    await click("Create vault");
    await expect.poll(() => target.textContent).toContain("Lock vault");
    await click("Lock vault");
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
    await unmount(component);
    target.remove();
    await start();
    await click(name);
    await wrongPassword();
    await correctPassword();
}, 15000);

it.each(["populated", "last-item-removed"])(
    "authenticates a %s vault before showing the editor",
    async (state) => {
        const name = `test-visible-${crypto.randomUUID()}`;
        const vault = await createVault(fixturePassword, name);
        const id = crypto.randomUUID();
        const secret = await vault.encryptAndStore(
            JSON.stringify({
                id,
                name: "Synthetic visible login",
                secret: "synthetic-item-password",
                createdAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
            }),
            id,
        );
        if (state === "last-item-removed") await vault.deleteSecret(secret.id);
        vault.close();
        await start();
        await click(name);
        await wrongPassword();
        input("password", fixturePassword);
        await click("Unlock");
        await expect.poll(() => target.textContent).toContain("Lock vault");
        if (state === "populated") {
            await expect
                .poll(() => target.textContent)
                .toContain("Synthetic visible login");
        }
        expect(target.textContent).not.toContain("synthetic-item-password");
        await click("Lock vault");
        await expect
            .poll(() => target.textContent)
            .toContain("Select a vault to unlock");
        expect(target.textContent).not.toContain("Synthetic visible login");
    },
    15000,
);

it("explains why a legacy empty vault cannot unlock without setting a password", async () => {
    const name = `test-visible-legacy-${crypto.randomUUID()}`;
    await legacyVault(name, false);
    await start();
    await click(name);
    for (const password of ["wrong-password", fixturePassword]) {
        input("password", password);
        await click("Unlock");
        await expect
            .poll(() => target.textContent)
            .toContain("original password cannot be verified");
        await expect.poll(() => button("Unlock").disabled).toBe(false);
        expect(target.textContent).toContain("create a separate new vault");
        expect(target.textContent).toContain("No password was set.");
        expect(target.textContent).not.toContain("Lock vault");
    }
}, 15000);
