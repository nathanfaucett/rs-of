import { render } from "svelte/server";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
const host = vi.hoisted(() => ({
    native: true,
    remote: false,
    invoke: vi.fn(),
    fetch: vi.fn(),
    idp: "https://node.test/idp",
    failure: undefined as string | undefined,
    state: "awaiting_choice",
    conflict: false,
}));
vi.mock("@tauri-apps/api/core", () => ({
    isTauri: () => host.native,
    invoke: host.invoke,
}));
vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
vi.mock("$lib/common/state/idpClient.svelte", () => ({
    getIdpApiUrl: () => host.idp,
    usesRemoteApis: () => !host.native || host.remote,
}));
vi.mock("$lib/common/state/deviceClient.svelte", () => ({
    getDeviceApiUrl: () => "https://node.test/device",
}));
vi.mock("$lib/common/state/oidc.svelte", () => ({ clearOidcSession: vi.fn() }));
vi.mock("$env/dynamic/public", () => ({
    env: { PUBLIC_STORAGE_BASE_URL: "https://node.test/storage" },
}));
const { load } = await import("./+page");
const { default: Installation } = await import("./+page.svelte");
const operation = {
    purpose: "create",
    operation_nonce: "1".repeat(32),
    administrator: "Alice",
    node: "My Node",
    idp: "https://node.test/idp",
    device: "https://node.test/device",
    storage: "https://node.test/storage",
    issuer: "https://node.test/idp",
    peer: null,
};
beforeEach(() => {
    vi.clearAllMocks();
    host.native = true;
    host.remote = false;
    host.failure = undefined;
    host.state = "awaiting_choice";
    host.conflict = false;
    host.invoke.mockImplementation(async () => ({
        base: "https://node.test",
        failure: host.failure,
    }));
    host.fetch.mockImplementation(async (url: string) => {
        const owner = ["idp", "device", "storage"].find((owner) =>
            url.startsWith(`https://node.test/${owner}/`),
        );
        if (!owner) throw new Error("Unexpected owner");
        if (url.endsWith("/setup/owner/status"))
            return Response.json(
                {
                    stage: "installation",
                    state: host.state,
                    ready: host.state === "ready",
                    owners: [owner],
                },
                { status: host.state === "unavailable" ? 503 : 200 },
            );
        if (url.endsWith("/setup/owner/public"))
            return Response.json({
                operation: {
                    ...operation,
                    operation_nonce:
                        host.conflict && owner === "storage"
                            ? "2".repeat(32)
                            : operation.operation_nonce,
                },
                owner,
                clients: [],
                registered: [],
                facts:
                    owner === "idp"
                        ? {
                              login_client:
                                  "01900000-0000-7000-8000-000000000001",
                          }
                        : {},
            });
        throw new Error("Retired or unexpected setup route");
    });
    vi.stubGlobal("fetch", host.fetch);
    vi.stubGlobal("window", { location: new URL("https://app.test") });
});
afterEach(() => vi.unstubAllGlobals());
it("observes each bundled owner directly and presents one owner-local Create/Join form without codes in status", async () => {
    const data = await load();
    expect(data.observation).toMatchObject({
        state: "needs_setup",
        target: { kind: "bundled" },
    });
    expect(host.fetch.mock.calls.map(([url]) => url)).toEqual(
        ["idp", "device", "storage"].map(
            (owner) => `https://node.test/${owner}/setup/owner/status`,
        ),
    );
    const { body } = render(Installation, { props: { data } });
    expect(body).toContain("Create an Installation");
    expect(body).toContain("Join through a trusted peer");
    expect(body).not.toContain("host setup code");
    expect(host.invoke).not.toHaveBeenCalledWith(
        "get_native_setup_code",
        expect.anything(),
    );
});
it.each(["creating", "joining"])(
    "acceptance %s is not readiness",
    async (state) => {
        host.state = state;
        expect((await load()).observation).toMatchObject({ state: "working" });
    },
);
it.each([false, true])(
    "correlates every owner before ready login in bundled/remote mode: %s",
    async (remote) => {
        host.remote = remote;
        host.state = "ready";
        const data = await load();
        expect(data.observation).toMatchObject({
            state: "ready",
            loginClient: { scopes: ["openid"] },
        });
        expect(render(Installation, { props: { data } }).body).toContain(
            'href="/signin"',
        );
        if (remote) expect(host.invoke).not.toHaveBeenCalled();
    },
);
it("does not accept unrelated ready owners", async () => {
    host.state = "ready";
    host.conflict = true;
    expect((await load()).observation.state).toBe("blocked");
});
it.each(["trust_consent", "trust_declined", "repair", "prerequisite"])(
    "keeps native %s actionable without sending setup requests",
    async (failure) => {
        host.failure = failure;
        const data = await load();
        expect(data.observation).toMatchObject({
            state: "blocked",
            action: failure,
        });
        expect(host.fetch).not.toHaveBeenCalled();
        expect(render(Installation, { props: { data } }).body).toContain(
            failure === "trust_consent"
                ? "Allow certificate trust"
                : "Retry observation",
        );
    },
);
it("remote selection ignores unrelated native startup failure", async () => {
    host.remote = true;
    host.failure = "repair";
    expect((await load()).observation.state).toBe("needs_setup");
    expect(host.invoke).not.toHaveBeenCalled();
});
it("unavailable owner evidence never becomes fresh permission", async () => {
    host.state = "unavailable";
    expect((await load()).observation).toMatchObject({
        state: "blocked",
        action: "prerequisite",
    });
});
