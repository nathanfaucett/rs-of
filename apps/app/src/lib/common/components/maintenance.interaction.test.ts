import { flushSync, mount, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import Maintenance from "./NativeSetupRecovery.svelte";
import MaintenancePage from "../../../routes/maintenance/+page.svelte";
const adapter = vi.hoisted(() => ({
    invoke: vi.fn(),
    reset: vi.fn(),
    remote: false,
    desktop: true,
}));
vi.mock("@tauri-apps/api/core", () => ({
    isTauri: () => adapter.desktop,
    invoke: adapter.invoke,
}));
vi.mock("../state/setupClient.svelte", () => ({
    resetDevice: adapter.reset,
    usesRemoteApis: () => adapter.remote,
}));
vi.mock("../state/oidc.svelte", () => ({
    getOidcClient: () => {
        throw new Error("Do not borrow an unrelated session");
    },
}));
vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
afterEach(async () => {
    if (component) await unmount(component);
    target?.remove();
    vi.resetAllMocks();
    adapter.remote = false;
    adapter.desktop = true;
    vi.unstubAllGlobals();
});
function render() {
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Maintenance, { target });
    flushSync();
}
function button(text: string) {
    return [...target.querySelectorAll("button")].find((node) =>
        node.textContent?.includes(text),
    )!;
}
function enter(label: string, value: string) {
    const input = [...target.querySelectorAll("label")]
        .find((node) => node.textContent?.includes(label))!
        .querySelector("input")!;
    input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
}
it("confirms local consequences, preserves failure and explicitly retries before reporting separate revocation", async () => {
    adapter.invoke.mockResolvedValue([
        {
            member_id: "retained",
            issuer: "https://installation.test/idp",
            status: "pending",
        },
    ]);
    adapter.reset
        .mockRejectedValueOnce(
            new Error("Provider refused; original evidence retained"),
        )
        .mockResolvedValueOnce(undefined);
    render();
    expect(target.textContent).toContain("does not erase peers");
    expect(button("Reset bundled Node").disabled).toBe(true);
    expect(adapter.reset).not.toHaveBeenCalled();
    enter("Type RESET", "RESET");
    button("Reset bundled Node").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Provider refused"),
    );
    expect(button("Reset bundled Node").disabled).toBe(true);
    enter("Type RESET", "RESET");
    button("Reset bundled Node").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Local cleanup complete"),
    );
    expect(target.textContent).toContain("pending");
    expect(adapter.reset).toHaveBeenCalledTimes(2);
});

it("keeps exact-reference recovery distinct and reports conflicting evidence without reset", async () => {
    adapter.invoke.mockImplementation(async (command) => {
        if (command === "recover_reset_references")
            throw new Error("Conflicting original reference");
        return [];
    });
    render();
    expect(target.querySelector("details summary")?.textContent).toContain(
        "Advanced exact-reference recovery",
    );
    button("Recover reset references").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Conflicting original reference"),
    );
    expect(adapter.reset).not.toHaveBeenCalled();
    expect(
        adapter.invoke.mock.calls.filter(
            ([command]) => command === "recover_reset_references",
        ),
    ).toHaveLength(1);
});
it("never borrows an ambient bearer and retains a partial peer failure for exact retry", async () => {
    const retained = [
        {
            member_id: "retained",
            issuer: "https://installation.test/idp",
            status: "pending",
        },
    ];
    adapter.invoke.mockImplementation(async (command) => {
        if (command === "revoke_reset_on_peer")
            throw new Error("Device owner unavailable; signer already revoked");
        return retained;
    });
    vi.stubGlobal("confirm", () => true);
    render();
    await vi.waitFor(() =>
        expect(button("Revoke on surviving peer")).toBeDefined(),
    );
    enter("Surviving peer", "https://peer.test/idp");
    button("Revoke on surviving peer").click();
    flushSync();
    expect(target.textContent).toContain("authorized User session");
    expect(
        adapter.invoke.mock.calls.some(
            ([command]) => command === "revoke_reset_on_peer",
        ),
    ).toBe(false);
    enter("Authorized User bearer", "transient-user-token");
    button("Revoke on surviving peer").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Device owner unavailable"),
    );
    expect(target.textContent).toContain("pending");
    expect(
        target.querySelector<HTMLInputElement>('input[type="password"]')?.value,
    ).toBe("");
    expect(adapter.invoke).toHaveBeenCalledWith("revoke_reset_on_peer", {
        peerApi: "https://peer.test/idp",
        bearer: "transient-user-token",
        memberId: "retained",
    });
    vi.unstubAllGlobals();
});
it("refuses local mutations when remote APIs are selected", async () => {
    adapter.remote = true;
    render();
    enter("Type RESET", "RESET");
    button("Reset bundled Node").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Selected remote APIs"),
    );
    expect(adapter.reset).not.toHaveBeenCalled();
    expect(adapter.invoke).not.toHaveBeenCalled();
});

it.each(["browser", "remote"])(
    "excludes local maintenance controls for %s and offers navigation back",
    async (kind) => {
        adapter.remote = kind === "remote";
        adapter.desktop = kind !== "browser";
        target = document.createElement("div");
        document.body.append(target);
        component = mount(MaintenancePage, { target });
        flushSync();
        expect(target.textContent).toContain("cannot reset them");
        expect(target.querySelector('a[href="/setup"]')).not.toBeNull();
        expect(target.querySelector("button")).toBeNull();
        expect(adapter.invoke).not.toHaveBeenCalled();
    },
);
it("refreshes retained targets only after authorized revocation acknowledgement", async () => {
    let acknowledged = false;
    adapter.invoke.mockImplementation(async (command) => {
        if (command === "revoke_reset_on_peer") {
            acknowledged = true;
            return;
        }
        return [
            {
                member_id: "retained",
                issuer: "https://installation.test/idp",
                status: acknowledged ? "acknowledged" : "pending",
            },
        ];
    });
    vi.stubGlobal("confirm", () => true);
    render();
    await vi.waitFor(() =>
        expect(button("Revoke on surviving peer")).toBeDefined(),
    );
    enter("Surviving peer", "https://peer.test/idp");
    enter("Authorized User bearer", "explicit-token");
    button("Revoke on surviving peer").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain(
            "acknowledged both signer and Device revocation",
        ),
    );
    expect(target.textContent).toContain("acknowledged");
    expect(adapter.reset).not.toHaveBeenCalled();
});
