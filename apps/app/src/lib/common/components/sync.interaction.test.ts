import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SyncPanel from "./SyncPanel.svelte";

vi.hoisted(() => {
    const values = new Map<string, string>();
    Object.defineProperty(globalThis, "localStorage", {
        value: {
            getItem: (key: string) => values.get(key) ?? null,
            setItem: (key: string, value: string) => values.set(key, value),
            clear: () => values.clear(),
            key: (index: number) => [...values.keys()][index] ?? null,
            get length() {
                return values.size;
            },
        },
        configurable: true,
    });
});
const session = vi.hoisted(() => ({
    token: "current-user",
    subject: "alice",
    revoked: false,
}));
vi.mock("../state/oidc.svelte", () => {
    const client = {
        registeredClientId: "app-client",
        getStoredTokenResponse: () => ({ access_token: session.token }),
        getVerifiedIdentity: async () => ({
            issuer: "https://node.test/idp",
            subject: session.subject,
            accessToken: session.token,
        }),
        getAccessToken: async () => session.token,
    };
    return { getOidcClient: () => client };
});
vi.mock("../state/idpClient.svelte", () => ({
    getIdpApiUrl: () => "https://node.test/idp",
}));
vi.mock("../state/storageApiUrl.svelte", () => ({
    getStorageApiUrl: () => "https://node.test/storage",
}));
vi.mock("../state/devices.svelte", () => ({
    listDevices: async () => [
        {
            id: "trusted",
            name: "Laptop",
            state: "approved",
            revokedAt: session.revoked ? 1 : null,
        },
        { id: "pending", name: "Pending phone", state: "pending" },
        {
            id: "revoked",
            name: "Revoked phone",
            state: "revoked",
            revokedAt: 1,
        },
        {
            id: "inconsistent",
            name: "Revoked laptop",
            state: "approved",
            revokedAt: 1,
        },
    ],
}));
vi.mock("../state/deviceClient.svelte", () => ({
    checkStorageSyncCapability: async (base: string) => {
        const reply = await fetch(`${base}/version`);
        if (!reply.ok) return false;
        return (await reply.json()).features.includes("sync");
    },
}));
let component: ReturnType<typeof mount> | undefined;
let target: HTMLDivElement;
const fetchApi = vi.fn();
beforeEach(() => {
    localStorage.clear();
    session.token = "current-user";
    session.subject = "alice";
    session.revoked = false;
    fetchApi.mockReset().mockImplementation(async (url: string) => {
        if (url.endsWith("/version"))
            return Response.json({ features: ["sync"] });
        if (url.endsWith("/oauth2/token"))
            return Response.json({
                access_token: "storage-user",
                token_type: "Bearer",
            });
        if (url.endsWith("/databases"))
            return Response.json([{ id: "database-one", name: "Notes" }]);
        if (url.endsWith("/filesystems"))
            return Response.json([{ id: "filesystem-one", name: "Documents" }]);
        throw new Error(`Unexpected URL: ${url}`);
    });
    vi.stubGlobal("fetch", fetchApi);
    vi.stubGlobal(
        "WebSocket",
        vi.fn(() => {
            throw new Error("No owner binding");
        }),
    );
});
afterEach(async () => {
    if (component) await unmount(component);
    component = undefined;
    target?.remove();
    vi.unstubAllGlobals();
});
function open() {
    target = document.createElement("div");
    document.body.append(target);
    component = mount(SyncPanel, { target });
    flushSync();
}
function button(label: string) {
    const node = [...target.querySelectorAll("button")].find(
        (entry) => entry.textContent?.trim() === label,
    );
    if (!node) throw new Error(`Missing button: ${label}`);
    return node;
}
it("checks a configured service, lists scoped resources and trusted Devices, and never calls attach a backup", async () => {
    open();
    await vi.waitFor(() => expect(button("Sync").disabled).toBe(false));
    button("Sync").click();
    await vi.waitFor(() => expect(target.textContent).toContain("Notes"));
    expect(target.textContent).toContain("Documents");
    expect(target.textContent).toContain("Laptop");
    expect(target.textContent).not.toContain("Revoked phone");
    expect(target.textContent).not.toContain("Pending phone");
    expect(target.textContent).not.toContain("Revoked laptop");
    expect(target.textContent).toContain("Resource Selections are unknown");
    expect(
        target.querySelectorAll('input[type="checkbox"]:checked'),
    ).toHaveLength(0);
    (
        target.querySelector('input[type="checkbox"]') as HTMLInputElement
    ).click();
    flushSync();
    button("Check selected targets").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Blocked — no data transferred"),
    );
    expect(WebSocket).not.toHaveBeenCalled();
    const exchanges = fetchApi.mock.calls.filter(([url]) =>
        url.endsWith("/oauth2/token"),
    );
    expect(
        exchanges.map(([, init]) =>
            (init.body as URLSearchParams).get("resource"),
        ),
    ).toEqual([
        "storage:databases",
        "storage:filesystems",
        "storage:databases",
        "storage:filesystems",
    ]);
    expect(
        exchanges.every(
            ([, init]) =>
                (init.body as URLSearchParams).get("subject_token") ===
                "current-user",
        ),
    ).toBe(true);
    const saved = Array.from({ length: localStorage.length }, (_, index) =>
        localStorage.getItem(localStorage.key(index) ?? ""),
    ).join();
    expect(saved).not.toContain("storage-user");
    expect(saved).not.toContain("current-user");
});
it("offers setup for an incapable service and rejects unsafe URLs before a request", async () => {
    fetchApi.mockResolvedValue(Response.json({ features: [] }));
    open();
    await vi.waitFor(() => expect(button("Sync").disabled).toBe(false));
    button("Sync").click();
    await vi.waitFor(() => expect(target.textContent).toContain("Set up sync"));
    expect(target.querySelector('a[href="#device-pairing"]')).not.toBeNull();
    const input = target.querySelector('input[type="url"]') as HTMLInputElement;
    input.value = "https://secret:password@node.test/storage?token=secret";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    button("Save and check").click();
    await vi.waitFor(() =>
        expect(target.querySelector('[role="alert"]')?.textContent).toContain(
            "HTTPS",
        ),
    );
    expect(fetchApi).toHaveBeenCalledTimes(1);
    expect([null, "null"]).toContain(localStorage.getItem("sync-target-url"));
});

it("restores Device preferences and blocked status, but not another User's status", async () => {
    open();
    await vi.waitFor(() => expect(button("Sync").disabled).toBe(false));
    button("Sync").click();
    await vi.waitFor(() => expect(target.textContent).toContain("Notes"));
    (
        target.querySelector('input[type="checkbox"]') as HTMLInputElement
    ).click();
    flushSync();
    button("Check selected targets").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Last local checks"),
    );
    if (component) await unmount(component);
    target.remove();
    open();
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Last local checks"),
    );
    expect(target.textContent).not.toContain("Last successful sync:");
    button("Sync").click();
    await vi.waitFor(() =>
        expect(
            target.querySelector('input[type="checkbox"]:checked'),
        ).not.toBeNull(),
    );
    if (component) await unmount(component);
    target.remove();
    session.subject = "bob";
    open();
    await vi.waitFor(() => expect(button("Sync").disabled).toBe(false));
    expect(target.textContent).not.toContain("Last local checks");
});
it("rechecks trust before saving a target result and removes a newly revoked Device", async () => {
    open();
    await vi.waitFor(() => expect(button("Sync").disabled).toBe(false));
    button("Sync").click();
    await vi.waitFor(() => expect(target.textContent).toContain("Notes"));
    (
        target.querySelector('input[type="checkbox"]') as HTMLInputElement
    ).click();
    flushSync();
    session.revoked = true;
    button("Check selected targets").click();
    await vi.waitFor(() =>
        expect(target.querySelector('[role="alert"]')?.textContent).toContain(
            "Targets or access changed",
        ),
    );
    expect(target.querySelector('input[type="checkbox"]')).toBeNull();
    expect(target.textContent).not.toContain("Last local checks");
    expect(WebSocket).not.toHaveBeenCalled();
});
it("does not use a prior session to check targets", async () => {
    open();
    await vi.waitFor(() => expect(button("Sync").disabled).toBe(false));
    button("Sync").click();
    await vi.waitFor(() => expect(target.textContent).toContain("Notes"));
    (
        target.querySelector('input[type="checkbox"]') as HTMLInputElement
    ).click();
    flushSync();
    const requests = fetchApi.mock.calls.length;
    session.token = "another-user";
    button("Check selected targets").click();
    await vi.waitFor(() =>
        expect(target.querySelector('[role="alert"]')?.textContent).toContain(
            "Targets or access changed",
        ),
    );
    expect(fetchApi).toHaveBeenCalledTimes(requests);
    expect(target.textContent).not.toContain("Last local checks");
});

it("saves a capable override, retains it on remount, and keeps source discovery on the configured API", async () => {
    open();
    await vi.waitFor(() => expect(button("Sync").disabled).toBe(false));
    button("Configure sync").click();
    flushSync();
    const input = target.querySelector('input[type="url"]') as HTMLInputElement;
    input.value = "https://backup.test/custom-storage/";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    button("Save and check").click();
    await vi.waitFor(() => expect(target.textContent).toContain("Notes"));
    expect(localStorage.getItem("sync-target-url")).toBe(
        '"https://backup.test/custom-storage"',
    );
    expect(
        fetchApi.mock.calls
            .filter(([url]) => url.startsWith("https://backup.test"))
            .map(([url]) => url),
    ).toEqual(["https://backup.test/custom-storage/version"]);
    if (component) await unmount(component);
    target.remove();
    open();
    await vi.waitFor(() => expect(button("Sync").disabled).toBe(false));
    button("Sync").click();
    await vi.waitFor(() => expect(target.textContent).toContain("Notes"));
    expect(
        fetchApi.mock.calls.filter(
            ([url]) => url === "https://backup.test/custom-storage/version",
        ),
    ).toHaveLength(2);
    button("Configure sync").click();
    flushSync();
    button("Use configured Storage API").click();
    await vi.waitFor(() => expect(button("Sync").disabled).toBe(false));
});
