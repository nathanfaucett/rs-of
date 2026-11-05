import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import PasswordGenerator from "./PasswordGenerator.svelte";

let component: ReturnType<typeof mount> | undefined;
let target: HTMLDivElement;
function button(name: string) {
    const found = [...target.querySelectorAll("button")].find(
        (element) => element.textContent?.trim() === name,
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
function control(label: string) {
    const found = [...target.querySelectorAll("label")].find(
        (element) =>
            (
                element.querySelector(".field-label-text")?.textContent ??
                element.textContent
            )?.trim() === label,
    );
    if (!found) throw new Error(`Missing label: ${label}`);
    return (
        found.htmlFor
            ? target.querySelector(`[id="${found.htmlFor}"]`)
            : found.querySelector("input")
    ) as HTMLInputElement | HTMLSelectElement;
}
function set(label: string, value: string) {
    const input = control(label);
    input.value = value;
    input.dispatchEvent(
        new Event(input.tagName === "SELECT" ? "change" : "input", {
            bubbles: true,
        }),
    );
    flushSync();
}
function start() {
    target = document.createElement("div");
    document.body.append(target);
    component = mount(PasswordGenerator, { target });
    flushSync();
}
afterEach(async () => {
    vi.restoreAllMocks();
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
});

it("focuses the generator settings when opened", () => {
    start();
    expect(document.activeElement).toBe(control("Generate type"));
});

it("shows invalid settings, clears stale values on settings change, and generates a configured passphrase", async () => {
    start();
    set("Password length", "");
    expect(button("Generate").disabled).toBe(true);
    expect(target.querySelector('[role="alert"]')?.textContent).toContain(
        "whole number",
    );
    set("Password length", "8");
    for (const label of [
        "Lowercase (a–z)",
        "Uppercase (A–Z)",
        "Digits (0–9)",
        "Symbols (!@#$%&*?+-=_.)",
    ]) {
        control(label).click();
        flushSync();
    }
    expect(button("Generate").disabled).toBe(true);
    expect(target.textContent).toContain("Select at least one");
    set("Generate type", "passphrase");
    set("Word count", "4");
    set("Word separator", "_");
    await click("Generate");
    await click("Show Generated value");
    const value = button("Hide Generated value")
        .closest("fieldset")
        ?.querySelector("p")?.textContent;
    expect(value).toMatch(/^[a-z]+_[a-z]+_[a-z]+_[a-z]+$/);
    set("Word count", "5");
    expect(target.textContent).not.toContain(value);
    expect(button("Copy Generated value").disabled).toBe(true);
    await click("Generate");
    window.dispatchEvent(new Event("pagehide"));
    flushSync();
    expect(button("Copy Generated value").disabled).toBe(true);
});

it("reports denied clipboard access without the value or exception, and resets reveal and feedback", async () => {
    start();
    const write = vi
        .spyOn(navigator.clipboard, "writeText")
        .mockRejectedValue(new Error("PRIVATE-EXCEPTION"));
    await click("Generate");
    await click("Copy Generated value");
    await expect.poll(() => target.textContent).toContain("Copy failed.");
    const value = write.mock.calls[0][0];
    expect(target.innerHTML).not.toContain(value);
    expect(target.textContent).not.toContain("PRIVATE-EXCEPTION");
    await click("Show Generated value");
    expect(target.textContent).toContain(value);
    await click("Generate");
    expect(target.textContent).not.toContain(value);
    expect(target.textContent).not.toContain("Copy failed.");
    expect(button("Show Generated value").getAttribute("aria-pressed")).toBe(
        "false",
    );
    await click("Clear generated value");
    expect(button("Copy Generated value").disabled).toBe(true);
});

it("fails closed when secure randomness fails, without revealing the exception", async () => {
    start();
    vi.spyOn(crypto, "getRandomValues").mockImplementation(() => {
        throw new Error("PRIVATE-RNG-ERROR");
    });
    await click("Generate");
    expect(target.textContent).toContain("Generation failed.");
    expect(target.textContent).not.toContain("PRIVATE-RNG-ERROR");
    expect(button("Copy Generated value").disabled).toBe(true);
});
