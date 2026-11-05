import { flushSync, mount, tick, unmount } from "svelte";
import { expect, it } from "vitest";
import Page from "./+page.svelte";

it("creates colliding vaults, selects their visible IDs, and reopens isolated records through the real workflow", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const component = mount(Page, { target });
    flushSync();
    function button(name: string) {
        const value = [...target.querySelectorAll("button")].find(
            (b) => b.textContent?.trim() === name,
        );
        if (!value) throw new Error(`Missing button: ${name}`);
        return value;
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
    const suffix = crypto.randomUUID();
    const names = [
        `Personal ${suffix}`,
        `personal!${suffix}`,
        `Personal ${suffix}`,
    ];
    const choices: string[] = [];
    try {
        await expect
            .poll(() => target.textContent)
            .toContain("Select a vault to unlock");
        for (const [index, name] of names.entries()) {
            await click("Create new vault");
            fill("vault-name", name);
            fill("password", `password-${index}`);
            fill("confirm", `password-${index}`);
            await click("Create vault");
            await expect.poll(() => target.textContent).toContain("Lock vault");
            await expect.poll(() => button("New secret").disabled).toBe(false);
            await click("New secret");
            fill("name", `Isolated login ${index}`);
            fill("secret", `secret-${index}`);
            fill("confirm-secret", `secret-${index}`);
            await click("Save");
            await expect.poll(() => target.textContent).toContain("Saved.");
            await click("Lock vault");
            await expect
                .poll(() => target.textContent)
                .toContain("Select a vault to unlock");
            const choice = [...target.querySelectorAll("button")].find(
                (b) =>
                    b.querySelector(".vault-name")?.textContent === name &&
                    !choices.includes(b.textContent?.trim() ?? ""),
            );
            if (!choice?.textContent)
                throw new Error("Missing created vault identity");
            choices.push(choice.textContent.trim());
        }
        expect(new Set(choices).size).toBe(3);
        for (const [index, choice] of choices.entries()) {
            await click(choice);
            fill("password", `password-${index}`);
            await click("Unlock");
            await expect
                .poll(() => target.textContent)
                .toContain(`Isolated login ${index}`);
            for (const other of [0, 1, 2].filter((v) => v !== index))
                expect(target.textContent).not.toContain(
                    `Isolated login ${other}`,
                );
            await click("Lock vault");
            await expect
                .poll(() => target.textContent)
                .toContain("Select a vault to unlock");
        }
    } finally {
        await unmount(component);
        target.remove();
    }
}, 30000);
