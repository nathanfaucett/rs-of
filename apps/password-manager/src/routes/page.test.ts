import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
    createVault,
    listVaults,
    unlockVault,
    type VaultHandle,
} from "$lib/vault/vault";
import Page from "./+page.svelte";

vi.mock("$lib/vault/vault", () => ({
    createVault: vi.fn(),
    unlockVault: vi.fn(),
    listVaults: vi.fn(),
}));

let target: HTMLDivElement;
let component: ReturnType<typeof mount>;
let close: ReturnType<typeof vi.fn>;

function button(name: string) {
    const value = [...target.querySelectorAll("button")].find(
        (element) =>
            element.querySelector(".vault-name")?.textContent === name ||
            element.textContent?.trim() === name,
    );
    if (!value) throw new Error(`Missing button: ${name}`);
    return value;
}

async function click(name: string) {
    const value = button(name);
    expect(value.disabled).toBe(false);
    value.click();
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

async function selector() {
    await expect
        .poll(() => target.textContent)
        .toContain("Select a vault to unlock");
    expect(button("personal")).toBeDefined();
    expect(button("work")).toBeDefined();
    expect(target.querySelector("#password")).toBeNull();
    expect(target.textContent).not.toContain("Lock vault");
}

beforeEach(async () => {
    vi.resetAllMocks();
    close = vi.fn();
    const vault = {
        id: crypto.randomUUID(),
        close,
        prepareItems: vi.fn(async () => {}),
        loadSecrets: vi.fn(async () => []),
    } as unknown as VaultHandle;
    vi.mocked(listVaults).mockResolvedValue([
        {
            id: "personal-id",
            name: "personal",
            databaseName: "personal-db",
            status: "ready",
        },
        {
            id: "work-id",
            name: "work",
            databaseName: "work-db",
            status: "ready",
        },
    ]);
    vi.mocked(unlockVault).mockResolvedValue(vault);
    vi.mocked(createVault).mockResolvedValue(vault);
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Page, { target });
    flushSync();
    await selector();
});

afterEach(async () => {
    await unmount(component);
    target.remove();
});

describe("vault page navigation", () => {
    it("shows actionable registry errors and permits a selector retry", async () => {
        await click("personal");
        vi.mocked(listVaults).mockRejectedValueOnce(
            new Error(
                "Allow browser storage and reload. Do not clear browser data.",
            ),
        );
        await click("Back");
        await expect
            .poll(() => target.textContent)
            .toContain("Do not clear browser data");
        await click("Try again");
        await selector();
    });
    it("selects duplicate names by their visible stable identity", async () => {
        vi.mocked(listVaults).mockResolvedValue([
            {
                id: "first-id",
                name: "Same",
                databaseName: "first-db",
                status: "ready",
            },
            {
                id: "second-id",
                name: "Same",
                databaseName: "second-db",
                status: "pending",
            },
        ]);
        await click("personal");
        await click("Back");
        await expect.poll(() => target.textContent).toContain("second-id");
        const choice = [...target.querySelectorAll("button")].find((b) =>
            b.textContent?.includes("second-id"),
        );
        expect(choice?.textContent).toContain("Registration interrupted");
        if (!choice) throw new Error("Missing vault identity choice");
        choice.click();
        flushSync();
        input("password", "second-password");
        await click("Unlock");
        await expect.poll(() => target.textContent).toContain("Lock vault");
        expect(unlockVault).toHaveBeenCalledExactlyOnceWith(
            "second-password",
            "second-id",
        );
    });

    it("shows storage errors without claiming successful creation", async () => {
        vi.mocked(createVault).mockRejectedValueOnce(
            new Error(
                "Cannot access the local vault registry. Allow browser storage.",
            ),
        );
        await click("Create new vault");
        input("vault-name", "personal");
        input("password", "master-password");
        input("confirm", "master-password");
        await click("Create vault");
        await expect
            .poll(() => target.textContent)
            .toContain("Allow browser storage");
        expect(target.textContent).not.toContain("Lock vault");
        expect(button("Cancel").disabled).toBe(false);
    });
    it("locks an unlocked vault and reloads the selector", async () => {
        await click("personal");
        input("password", "master-password");
        await click("Unlock");
        await expect.poll(() => target.textContent).toContain("Lock vault");
        expect(unlockVault).toHaveBeenCalledExactlyOnceWith(
            "master-password",
            "personal-id",
        );

        vi.mocked(listVaults).mockResolvedValue([
            {
                id: "personal-id",
                name: "personal",
                databaseName: "personal-db",
                status: "ready",
            },
            {
                id: "work-id",
                name: "work",
                databaseName: "work-db",
                status: "ready",
            },
            {
                id: "family-id",
                name: "family",
                databaseName: "family-db",
                status: "ready",
            },
        ]);
        await click("Lock vault");
        await selector();
        expect(button("family")).toBeDefined();
        expect(close).toHaveBeenCalledTimes(1);
        expect(listVaults).toHaveBeenCalledTimes(2);
        expect(target.textContent).not.toContain(
            "Stored locally in your encrypted vault.",
        );
    });

    it("locks a newly created vault and returns to the selector", async () => {
        await click("Create new vault");
        input("vault-name", " family ");
        input("password", "master-password");
        input("confirm", "master-password");
        await click("Create vault");
        await expect.poll(() => target.textContent).toContain("Lock vault");
        expect(createVault).toHaveBeenCalledExactlyOnceWith(
            "master-password",
            "family",
        );

        await click("Lock vault");
        await selector();
        expect(close).toHaveBeenCalledTimes(1);
    });

    it("Back returns to the selector and clears the unlock password and error", async () => {
        await click("personal");
        vi.mocked(unlockVault).mockRejectedValueOnce(
            new Error("Wrong password."),
        );
        input("password", "wrong-password");
        await click("Unlock");
        await expect
            .poll(() => target.textContent)
            .toContain("Wrong password.");
        await click("Back");
        await selector();
        await click("work");
        expect(target.textContent).toContain("Unlock vault: work");
        expect(target.querySelector<HTMLInputElement>("#password")?.value).toBe(
            "",
        );
        expect(target.textContent).not.toContain("Wrong password.");
        expect(close).not.toHaveBeenCalled();
        expect(createVault).not.toHaveBeenCalled();
    });

    it("Cancel returns to the selector and clears all create fields and errors", async () => {
        await click("Create new vault");
        input("vault-name", "discarded");
        input("password", "master-password");
        input("confirm", "different-password");
        await click("Create vault");
        expect(target.textContent).toContain("Passwords do not match.");
        await click("Cancel");
        await selector();
        await click("Create new vault");
        for (const id of ["vault-name", "password", "confirm"]) {
            expect(
                target.querySelector<HTMLInputElement>(`#${id}`)?.value,
            ).toBe("");
        }
        expect(target.textContent).not.toContain("Passwords do not match.");
        expect(createVault).not.toHaveBeenCalled();
        expect(unlockVault).not.toHaveBeenCalled();
        expect(close).not.toHaveBeenCalled();
    });
});
