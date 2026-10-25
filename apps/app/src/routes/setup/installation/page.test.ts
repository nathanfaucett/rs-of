import { render } from "svelte/server";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const host = vi.hoisted(() => ({
    native: true,
    remote: false,
    invoke: vi.fn(),
    fetch: vi.fn(),
    idp: "https://node.test/idp" as string | null,
    device: "https://policy.test/api",
    clientId: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({
    isTauri: () => host.native,
    invoke: host.invoke,
}));
vi.mock("$app/navigation", () => ({ goto: vi.fn(), invalidateAll: vi.fn() }));
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
vi.mock("$lib/common/state/oidc.svelte", () => ({
    getOidcClient: vi.fn(),
    ensureOidcClientId: host.clientId,
}));
vi.mock("$lib/common/state/idpClient.svelte", () => ({
    getIdpApiUrl: () => host.idp,
    ensureTauriIdpApiUrl: async () => host.idp,
    usesRemoteApis: () => !host.native || host.remote,
}));
vi.mock("$lib/common/state/deviceClient.svelte", () => ({
    getDeviceApiUrl: () => host.device,
}));
vi.mock("$env/dynamic/public", () => ({
    env: { PUBLIC_STORAGE_BASE_URL: "https://data.test/api" },
}));

const { load } = await import("./+page");
const { default: Installation } = await import("./+page.svelte");
const login = {
    client_id: "01900000-0000-7000-8000-000000000001",
    scopes: ["openid"],
};
function status(state = "awaiting_choice") {
    return {
        stage: "installation",
        state,
        ready: state === "ready",
        owners: ["idp", "device", "storage"],
        ...(state === "ready" ? { login_client: login } : {}),
    };
}
function native(state = "awaiting_choice", device = false) {
    return {
        base: "https://node.test",
        status: status(state),
        device_setup_required: device,
    };
}
beforeEach(() => {
    vi.clearAllMocks();
    host.native = true;
    host.remote = false;
    host.idp = "https://node.test/idp";
    host.fetch.mockReset();
    host.invoke.mockReset().mockResolvedValue(native());
    vi.stubGlobal("fetch", host.fetch);
    vi.stubGlobal("window", {
        location: new URL("https://app.test"),
        localStorage: { getItem: () => null },
    });
});
afterEach(() => vi.unstubAllGlobals());

it("observes bundled setup through one secret-free target-bound snapshot", async () => {
    const data = await load();
    expect(data.observation).toMatchObject({
        state: "needs_setup",
        target: { kind: "bundled", idp: "https://node.test/idp" },
        action: "choose",
    });
    expect(host.invoke.mock.calls).toEqual([["get_onboarding_host"]]);
    expect(host.fetch).not.toHaveBeenCalled();
    expect(JSON.stringify(data.observation)).not.toContain("setup_code");
    const { body } = render(Installation, { props: { data } });
    expect(body).toContain("Create an Installation");
    expect(body).toContain("Join through a trusted peer");
    expect(body).not.toContain("Get bundled host code");
    expect(body).not.toContain("Node name");
    expect(body).toContain("Advanced connection settings");
    expect(body).toContain('type="password"');
});

it.each(["creating", "joining"])(
    "shows %s as working, never readiness from acceptance",
    async (state) => {
        host.invoke.mockResolvedValue(native(state));
        const data = await load();
        expect(data.observation).toMatchObject({
            state: "working",
            action: state === "joining" ? "approve" : "observe",
        });
        const { body } = render(Installation, { props: { data } });
        expect(body).toContain('aria-label="Setup in progress"');
        expect(body).not.toContain('href="/signin"');
        if (state === "joining")
            expect(body).toContain("Devices → Add a device");
    },
);

it.each([false, true])(
    "preserves verified readiness and Device preferences without authenticating: %s",
    async (device) => {
        host.invoke.mockResolvedValue(native("ready", device));
        const data = await load();
        expect(data.observation).toMatchObject({
            state: "ready",
            action: "sign_in",
            loginClient: login,
        });
        const { body } = render(Installation, { props: { data } });
        expect(body).toContain(
            'href="/signin"',
        );
        expect(body).not.toContain("<form");
        expect(host.clientId).not.toHaveBeenCalled();
    },
);

it("shows startup without inventing owner readiness", async () => {
    host.invoke.mockResolvedValue({});
    const data = await load();
    expect(data.observation.state).toBe("starting");
    const { body } = render(Installation, { props: { data } });
    expect(body).toContain("Starting bundled services");
    expect(body).not.toContain("<form");
});

it.each(["retry", "prerequisite", "repair"] as const)(
    "shows safe %s instructions, not a default destructive recovery",
    async (action) => {
        host.invoke.mockResolvedValue({ failure: action });
        const data = await load();
        expect(data.observation).toMatchObject({ state: "blocked", action });
        const { body } = render(Installation, { props: { data } });
        expect(body).toContain("Retry observation");
        expect(body).not.toContain("<form");
        expect(body).not.toContain('href="/signin"');
        expect(body).toContain('href="/maintenance"');
        expect(body).not.toContain("Type RESET");
        expect(body).not.toContain("Recover reset references</button>");
        expect(body).not.toContain("Revoke on surviving peer</button>");
        if (action === "repair")
            expect(body).toContain("Preserve the existing identity");
    },
);

it.each([
    null,
    { ...status("ready"), login_client: null },
    { ...status("ready"), login_client: { ...login, scopes: [] } },
    { ...status(), ready: true },
    { ...status("ready"), owners: ["idp"] },
    { ...status(), state: "invented" },
])(
    "blocks malformed native evidence without manufacturing readiness: %j",
    async (evidence) => {
        host.invoke.mockResolvedValue({
            base: "https://node.test",
            status: evidence,
            device_setup_required: false,
        });
        const data = await load();
        expect(data.observation.state).toBe("blocked");
        const { body } = render(Installation, { props: { data } });
        expect(body).not.toContain('href="/signin"');
        expect(body).not.toContain("<form");
    },
);

it.each(["unfinished", "unavailable"])(
    "routes damaged shared %s evidence to repair",
    async (state) => {
        host.invoke.mockResolvedValue(native(state));
        expect((await load()).observation).toMatchObject({
            state: "blocked",
            action: state === "unavailable" ? "prerequisite" : "repair",
        });
    },
);

it.each([true, false])(
    "honors remote selection independently of failed bundled services, native=%s",
    async (desktop) => {
        host.native = desktop;
        host.remote = true;
        host.invoke.mockRejectedValue(new Error("bundled host failed"));
        host.fetch.mockResolvedValue(Response.json(status("ready")));
        const data = await load();
        expect(data.observation).toMatchObject({
            state: "ready",
            target: { kind: "remote", idp: host.idp },
            loginClient: login,
        });
        const { body } = render(Installation, { props: { data } });
        expect(body).toContain('href="/signin"');
        expect(body).not.toContain("Get bundled host code");
        expect(body).not.toContain("Reset bundled Node");
        expect(host.invoke).not.toHaveBeenCalled();
        expect(host.fetch).toHaveBeenCalledTimes(1);
    },
);

it("discards readiness from a target changed during HTTP observation", async () => {
    host.remote = true;
    host.fetch.mockImplementation(async () => {
        host.idp = "https://other.test/idp";
        return Response.json(status("ready"));
    });
    const data = await load();
    expect(data.observation).toMatchObject({
        state: "blocked",
        action: "retry",
        target: { idp: "https://other.test/idp" },
        hosts: [],
    });
});

it("discards bundled readiness when remote selection changes during native observation", async () => {
    host.invoke.mockImplementation(async () => {
        host.remote = true;
        host.idp = "https://other.test/idp";
        return native("ready");
    });
    expect((await load()).observation).toMatchObject({
        state: "blocked",
        target: { kind: "remote", idp: "https://other.test/idp" },
        hosts: [],
    });
});

it("keeps browser transport failure actionable without native recovery", async () => {
    host.native = false;
    host.fetch.mockRejectedValue(new Error("offline"));
    const data = await load();
    expect(data.observation).toMatchObject({
        state: "blocked",
        action: "retry",
    });
    const { body } = render(Installation, { props: { data } });
    expect(body).toContain("Retry observation");
    expect(body).not.toContain("Reset bundled Node");
    expect(host.invoke).not.toHaveBeenCalled();
});

it("keeps separate configured independent owner admission scopes", async () => {
    host.native = false;
    host.fetch.mockImplementation(async (url: string) =>
        Response.json({
            ...status(),
            owners: [
                url.startsWith(host.idp!)
                    ? "idp"
                    : url.startsWith(host.device)
                      ? "device"
                      : "storage",
            ],
        }),
    );
    const data = await load();
    expect(data.observation.hosts.map((item) => item.status.owners)).toEqual([
        ["idp"],
        ["device"],
        ["storage"],
    ]);
    const { body } = render(Installation, { props: { data } });
    expect(body).toContain("https://policy.test/api");
    expect(body).toContain("https://data.test/api");
    expect(host.invoke).not.toHaveBeenCalled();
});

it("routes direct sign-in using the same observation and passes its verified login evidence", async () => {
    const { load: signin } = await import("../../signin/+page");
    host.invoke.mockResolvedValue(native("ready"));
    await signin();
    expect(host.clientId).toHaveBeenCalledWith(
        expect.objectContaining({ state: "ready", loginClient: login }),
    );
    expect(host.invoke.mock.calls).toEqual([["get_onboarding_host"]]);
    host.invoke.mockRejectedValue(new Error("offline"));
    await expect(signin()).rejects.toMatchObject({
        location: "/setup/installation",
    });
});

it("offers retry for a transient remote server failure, not identity repair", async () => {
    host.native = false;
    host.fetch.mockResolvedValue(new Response(null, { status: 500 }));
    expect((await load()).observation).toMatchObject({
        state: "blocked",
        action: "retry",
    });
});

it("does not disclose embedded credentials or query secrets from invalid selected addresses", async () => {
    host.remote = true;
    host.idp =
        "https://operator:private-password@node.test/idp?setup=private-code";
    const data = await load();
    expect(data.observation.state).toBe("blocked");
    expect(JSON.stringify(data.observation)).not.toContain("private-password");
    expect(JSON.stringify(data.observation)).not.toContain("private-code");
    expect(host.fetch).not.toHaveBeenCalled();
    expect(host.invoke).not.toHaveBeenCalled();
});

it("discards a stale target's malformed evidence rather than requesting repair of the new target", async () => {
    host.remote = true;
    host.fetch.mockImplementation(async () => {
        host.idp = "https://new.test/idp";
        return Response.json({ ready: true });
    });
    expect((await load()).observation).toMatchObject({
        state: "blocked",
        action: "retry",
        target: { idp: "https://new.test/idp" },
        hosts: [],
    });
});

it("allows deferred Device preferences after verified readiness", async () => {
    host.invoke.mockResolvedValue({
        base: "https://node.test",
        status: status("ready"),
    });
    expect((await load()).observation).toMatchObject({
        state: "ready",
        action: "sign_in",
    });
});


it.each(["matching", "nonce", "issuer", "base", "login"])(
    "renders completed independent owners using correlated public evidence: %s",
    async (variation) => {
        host.native = false;
        const bases = { idp: host.idp!, device: host.device, storage: "https://data.test/api" };
        const operation = { purpose: "create", operation_nonce: "1".repeat(32), administrator: "Alice", node: "Node", ...bases, issuer: bases.idp, peer: null };
        host.fetch.mockImplementation(async (url: string, init: RequestInit) => {
            expect(init.headers).not.toHaveProperty("Authorization");
            if (url === bases.idp + "/setup/status") return new Response(null, { status: 404 });
            const owner = (["idp", "device", "storage"] as const).find(owner => url.startsWith(bases[owner] + "/"))!;
            if (url.endsWith("/status")) return Response.json({ stage: "installation", state: "ready", ready: true, owners: [owner] });
            const exact = { ...operation };
            if (owner === "storage") {
                if (variation === "nonce") exact.operation_nonce = "2".repeat(32);
                if (variation === "issuer") exact.issuer = "https://wrong.test/idp";
                if (variation === "base") exact.device = "https://wrong.test/device";
            }
            return Response.json({ operation: exact, owner, clients: [], registered: [], facts: owner === "idp" && variation !== "login" ? { login_client: login.client_id } : {} });
        });
        const data = await load();
        const { body } = render(Installation, { props: { data } });
        if (variation === "matching") {
            expect(data.observation).toMatchObject({ state: "ready", action: "sign_in", loginClient: login });
            expect(body).toContain('href="/signin"');
            expect(body).not.toContain("needs repair");
        } else {
            expect(data.observation.state).toBe("blocked");
            expect(body).not.toContain('href="/signin"');
        }
        expect(host.invoke).not.toHaveBeenCalled();
    },
);
