import { flushSync, mount, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import JoinSetup from "./JoinSetup.svelte";
import type { OnboardingObservation } from "../state/onboarding";
const native = vi.hoisted(() => ({
    invoke: vi.fn(),
    idp: "https://target.test/idp",
    device: "https://target.test/device",
}));
vi.mock("@tauri-apps/api/core", () => ({
    isTauri: () => false,
    invoke: native.invoke,
}));
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
vi.mock("../state/oidc.svelte", () => ({ clearOidcSession: vi.fn() }));
vi.mock("../state/idpClient.svelte", () => ({
    getIdpApiUrl: () => native.idp,
    usesRemoteApis: () => true,
}));
vi.mock("../state/deviceClient.svelte", () => ({
    getDeviceApiUrl: () => native.device,
}));
vi.mock("$env/dynamic/public", () => ({ env: {} }));
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
const peer = "d".repeat(64);
const code = "a".repeat(64);
function status(state = "awaiting_choice", ready = false) {
    return {
        stage: "installation" as const,
        state: state as "awaiting_choice",
        ready,
        owners: ["idp", "device", "storage"] as (
            | "idp"
            | "device"
            | "storage"
        )[],
        ...(ready
            ? {
                  login_client: {
                      client_id: "01900000-0000-7000-8000-000000000001",
                      scopes: ["openid"],
                  },
              }
            : {}),
    };
}
function observation(state = "awaiting_choice"): OnboardingObservation {
    return {
        state: "needs_setup",
        action: "choose",
        target: {
            kind: "remote",
            idp: "https://target.test/idp",
            device: "https://target.test/device",
            storage: null,
        },
        hosts: [{ base: "https://target.test/idp", status: status(state) }],
    };
}
function enter(label: string, value: string) {
    const input = [...target.querySelectorAll("label")]
        .find((node) => node.textContent?.includes(label))
        ?.querySelector("input");
    if (!input) throw new Error("Missing field " + label);
    input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
}
function button(text: string) {
    const node = [...target.querySelectorAll("button")].find(
        (node) => node.textContent?.trim() === text,
    );
    if (!node) throw new Error("Missing button " + text);
    return node;
}
function render(value = observation()) {
    target = document.createElement("div");
    document.body.append(target);
    component = mount(JoinSetup, { target, props: { observation: value } });
    flushSync();
}
afterEach(async () => {
    if (component) await unmount(component);
    target?.remove();
    vi.unstubAllGlobals();
    native.invoke.mockReset();
    native.idp = "https://target.test/idp";
    native.device = "https://target.test/device";
});
it("uses one pinned peer and a default name, then shows the authenticated Installation before approval", async () => {
    let intent: Record<string, unknown>;
    const fetchApi = vi.fn(async (url: string, init: RequestInit = {}) => {
        if (url.endsWith("/intent"))
            return Response.json({
                ...intent,
                issuer: "https://installation.test/idp",
            });
        if (url.endsWith("/join")) {
            intent = JSON.parse(init.body as string);
            return Response.json(status("joining"), { status: 202 });
        }
        return Response.json(status("joining"));
    });
    vi.stubGlobal("fetch", fetchApi);
    render();
    expect(target.textContent).not.toContain("Node name");
    enter("Trusted peer endpoint ID", peer);
    enter("Host setup code", code);
    expect(button("Request approval and join").disabled).toBe(false);
    expect(target.querySelector("form")!.checkValidity()).toBe(true);
    button("Request approval and join").click();
    flushSync();
    await vi.waitFor(() =>
        expect(target.textContent).toContain(
            "Authenticated Installation: https://installation.test/idp",
        ),
    );
    const request = fetchApi.mock.calls.find(([url]) => url.endsWith("/join"))!;
    expect(request[0]).toBe("https://target.test/idp/setup/join");
    expect(JSON.parse(request[1]!.body as string)).toMatchObject({
        node: "My Node",
        peer,
    });
    expect(target.textContent).toContain("Devices → Add a device");
    expect(target.querySelector('a[href="/signin"]')).toBeNull();
    expect(native.invoke).not.toHaveBeenCalled();
    expect(
        target.querySelector<HTMLInputElement>('input[type="password"]')?.value,
    ).toBe("");
});

it("recovers after reload without replacement, retries exact identity and distinguishes cancellation from cleanup", async () => {
    const retained = {
        operation_nonce: "b".repeat(32),
        node: "Retained Node",
        peer,
        issuer: "https://installation.test/idp",
    };
    const fetchApi = vi.fn(async (url: string) => {
        if (url.endsWith("/intent")) return Response.json(retained);
        if (url.endsWith("/cancel")) return new Response(null, { status: 204 });
        return Response.json(status("joining"), {
            status: url.endsWith("/join") ? 202 : 200,
        });
    });
    vi.stubGlobal("fetch", fetchApi);
    render(observation("joining"));
    expect(button("Request approval and join").disabled).toBe(true);
    enter("Host setup code", code);
    button("Recover retained Join").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Authenticated Installation:"),
    );
    expect(target.textContent).toContain("Retained Node");
    enter("Host setup code", code);
    target
        .querySelector("form")!
        .dispatchEvent(
            new Event("submit", { bubbles: true, cancelable: true }),
        );
    await vi.waitFor(() =>
        expect(fetchApi.mock.calls.some(([url]) => url.endsWith("/join"))).toBe(
            true,
        ),
    );
    const request = fetchApi.mock.calls.find(([url]) => url.endsWith("/join"))!;
    expect(
        JSON.parse(
            (request as unknown as [string, RequestInit])[1].body as string,
        ),
    ).toEqual({
        operation_nonce: retained.operation_nonce,
        node: "Retained Node",
        peer,
    });
    await vi.waitFor(() =>
        expect(button("Cancel exact Join").disabled).toBe(true),
    );
    enter("Host setup code", code);
    button("Cancel exact Join").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain(
            "Cancellation signaled, not cleanup completion",
        ),
    );
    expect(target.querySelector('a[href="/signin"]')).toBeNull();
    expect(target.textContent).toContain("Authenticated Installation:");
});

it("does not expose sign-in until current public login verifies, including a source-offline reopen", async () => {
    let ready = false;
    const fetchApi = vi.fn(async (_url: string) =>
        Response.json(status(ready ? "ready" : "joining", ready)),
    );
    vi.stubGlobal("fetch", fetchApi);
    render(observation("joining"));
    button("Refresh status").click();
    await vi.waitFor(() => expect(fetchApi).toHaveBeenCalled());
    expect(target.querySelector('a[href="/signin"]')).toBeNull();
    await vi.waitFor(() =>
        expect(button("Refresh status").disabled).toBe(false),
    );
    ready = true;
    button("Refresh status").click();
    await vi.waitFor(() =>
        expect(target.querySelector('a[href="/signin"]')).not.toBeNull(),
    );
    expect(
        fetchApi.mock.calls.every(([url]) =>
            String(url).startsWith("https://target.test/"),
        ),
    ).toBe(true);
});

it.each(["idp", "device"] as const)(
    "refuses a changed remote %s target rather than sending the retained capability or provisioning bundled owners",
    async (owner) => {
        const fetchApi = vi.fn();
        vi.stubGlobal("fetch", fetchApi);
        render();
        enter("Trusted peer endpoint ID", peer);
        enter("Host setup code", code);
        native[owner] = "https://other.test/" + owner;
        target
            .querySelector("form")!
            .dispatchEvent(
                new Event("submit", { bubbles: true, cancelable: true }),
            );
        await vi.waitFor(() =>
            expect(target.textContent).toContain("selected target"),
        );
        expect(fetchApi).not.toHaveBeenCalled();
        expect(native.invoke).not.toHaveBeenCalled();
    },
);

it("keeps a timed-out Join uncertain and recovers the same operation before explicit retry", async () => {
    let accepted: Record<string, unknown>;
    const fetchApi = vi.fn(async (url: string, init: RequestInit = {}) => {
        if (url.endsWith("/join")) {
            accepted = JSON.parse(init.body as string);
            throw new Error("transport timeout after acceptance");
        }
        if (url.endsWith("/intent"))
            return Response.json({
                ...accepted,
                issuer: "https://installation.test/idp",
            });
        return Response.json(status("joining"));
    });
    vi.stubGlobal("fetch", fetchApi);
    render();
    enter("Trusted peer endpoint ID", peer);
    enter("Host setup code", code);
    target
        .querySelector("form")!
        .dispatchEvent(
            new Event("submit", { bubbles: true, cancelable: true }),
        );
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Join outcome is uncertain"),
    );
    expect(button("Retry exact Join").disabled).toBe(true);
    expect(target.querySelector('a[href="/signin"]')).toBeNull();
    enter("Host setup code", code);
    button("Recover retained Join").click();
    await vi.waitFor(() =>
        expect(target.textContent).toContain("Authenticated Installation:"),
    );
    expect(
        fetchApi.mock.calls.filter(([url]) => url.endsWith("/join")),
    ).toHaveLength(1);
    await unmount(component);
    component = undefined as never;
    expect(fetchApi.mock.calls.some(([url]) => url.endsWith("/cancel"))).toBe(
        false,
    );
});
it("shows rejected or interrupted work as needing attention and requires recovery, not a fresh identity", () => {
    render(observation("unfinished"));
    expect(target.textContent).toContain("Join stopped or was rejected");
    expect(button("Request approval and join").disabled).toBe(true);
    expect(target.textContent).toContain("Recover retained Join");
    expect(target.querySelector('a[href="/signin"]')).toBeNull();
});

it("rejects an unpinned peer without sending setup authorization", () => {
    const fetchApi = vi.fn();
    vi.stubGlobal("fetch", fetchApi);
    render();
    enter("Trusted peer endpoint ID", "https://untrusted.test");
    enter("Host setup code", code);
    expect(button("Request approval and join").disabled).toBe(true);
    target
        .querySelector("form")!
        .dispatchEvent(
            new Event("submit", { bubbles: true, cancelable: true }),
        );
    expect(fetchApi).not.toHaveBeenCalled();
});

it("observes current public login automatically when recovery finds completed work", async () => {
    let retained: Record<string, unknown>;
    const fetchApi = vi.fn(async (url: string, init: RequestInit = {}) => {
        if (url.endsWith("/join")) {
            retained = JSON.parse(init.body as string);
            return Response.json(status("joining"), { status: 202 });
        }
        if (url.endsWith("/intent"))
            return Response.json({
                ...retained,
                issuer: "https://installation.test/idp",
            });
        return Response.json(status("ready", true));
    });
    vi.stubGlobal("fetch", fetchApi);
    render();
    enter("Trusted peer endpoint ID", peer);
    enter("Host setup code", code);
    button("Request approval and join").click();
    await vi.waitFor(() =>
        expect(target.querySelector('a[href="/signin"]')).not.toBeNull(),
    );
});
