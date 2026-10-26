import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ApplicationSdk } from "./index.js";

const values = new Map<string, string>();
const metadata = {
    issuer: "https://installation.test",
    authorization_endpoint: "https://node.test/idp/oauth2/authorize",
    token_endpoint: "https://node.test/idp/oauth2/token",
    userinfo_endpoint: "https://node.test/idp/userinfo",
    end_session_endpoint: "https://node.test/idp/logout",
};
let subject = "alice";
const service = vi.fn(async (input: URL | RequestInfo, _init?: RequestInit) => {
    const url = String(input);
    if (url.endsWith("/.well-known/openid-configuration"))
        return Response.json(metadata);
    if (url.endsWith("/userinfo")) return Response.json({ sub: subject });
    if (url.endsWith("/oauth2/token"))
        return Response.json({
            access_token: "access",
            refresh_token: "refresh",
            expires_in: 3600,
        });
    throw new Error("Unexpected service request");
});
const options = (clientId = "web") => ({
    idpBaseUrl: "https://node.test/idp",
    storageBaseUrl: "https://files.test/storage",
    client: {
        clientId,
        redirectUri: "https://app.test/callback",
        responseType: "code",
        registration: {
            redirectUris: ["https://app.test/callback"],
            application: { uri: "app.test", name: "Example" },
            scope: "openid offline",
            tokenEndpointAuthMethod: "none" as const,
        },
    },
    fetch: service,
    disableNativeRequests: true,
});
beforeEach(() => {
    values.clear();
    subject = "alice";
    service.mockClear();
    vi.stubGlobal("window", {
        location: { href: "https://app.test/", origin: "https://app.test" },
        localStorage: {
            get length() {
                return values.size;
            },
            key: (i: number) => [...values.keys()][i] ?? null,
            getItem: (key: string) => values.get(key) ?? null,
            setItem: (key: string, value: string) => values.set(key, value),
            removeItem: (key: string) => values.delete(key),
        },
    });
});
afterEach(() => vi.unstubAllGlobals());
async function callback(sdk: ApplicationSdk) {
    const url = await sdk.getAuthorizationUrl();
    return new URL(
        `https://app.test/callback?code=one&state=${url.searchParams.get("state")}`,
    );
}
it("establishes a verified identity across full-page redirect and reload, keeping Application, client and issuer distinct", async () => {
    const first = new ApplicationSdk(options());
    const url = await callback(first);
    const returning = new ApplicationSdk(options());
    await returning.handleSigninCallback(url);
    expect(returning.session).toMatchObject({
        status: "authenticated",
        identity: {
            issuer: "https://installation.test",
            subject: "alice",
            application: "app.test",
            clientId: "web",
        },
    });
    expect(JSON.stringify(returning.session)).not.toContain("access");
    const reload = new ApplicationSdk(options());
    await reload.restore();
    expect(reload.session).toMatchObject({
        status: "authenticated",
        identity: { subject: "alice" },
    });
});

it.each([
    "",
    "?code=one",
    "?code=one&state=unknown",
    "#access_token=untrusted",
])("does not authenticate an invalid/no-op callback %s", async (query) => {
    const sdk = new ApplicationSdk(options());
    await expect(
        sdk.handleSigninCallback(new URL(`https://app.test/callback${query}`)),
    ).rejects.toThrow("callback");
    expect(sdk.session.status).toBe("unauthenticated");
    expect(
        service.mock.calls.some(([url]) => String(url).endsWith("/userinfo")),
    ).toBe(false);
});
it("isolates tokens and PKCE across clients on the same authority, including scoped clearing", async () => {
    const a = new ApplicationSdk(options("a"));
    const pendingA = await callback(a);
    const b = new ApplicationSdk(options("b"));
    await expect(b.handleSigninCallback(pendingA)).rejects.toThrow("callback");
    await a.handleSigninCallback(pendingA);
    const reloadB = new ApplicationSdk(options("b"));
    await reloadB.restore();
    expect(reloadB.session.status).toBe("unauthenticated");
    const b2 = new ApplicationSdk(options("b"));
    await b2.handleSigninCallback(await callback(b2));
    await a.signout();
    const reload = new ApplicationSdk(options("b"));
    await reload.restore();
    expect(reload.session.status).toBe("authenticated");
});
it("refreshes through OIDC without retiring the same verified context, but ends a changed User", async () => {
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const snapshot = sdk.session;
    const token = sdk.tokenProvider();
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 3600000);
    try {
        await expect(token()).resolves.toBe("access");
        expect(sdk.session).toBe(snapshot);
        expect(
            service.mock.calls.some(
                ([, init]) =>
                    new URLSearchParams(init?.body as string).get(
                        "grant_type",
                    ) === "refresh_token",
            ),
        ).toBe(true);
        subject = "bob";
        await expect(token()).rejects.toThrow("identity");
        expect(sdk.session.status).toBe("ended");
        await expect(token()).rejects.toThrow("ended");
    } finally {
        vi.restoreAllMocks();
    }
});
it("clears local credentials and notifies before failed provider discovery, without purging another client", async () => {
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const reload = new ApplicationSdk(options());
    // No discovery cache in this runtime: signing out must not await it to clear local state.
    const notifications: string[] = [];
    reload.subscribe((session) => notifications.push(session.status));
    service.mockRejectedValueOnce(
        new Error("offline with confidential details"),
    );
    const ending = reload.signout({ provider: true });
    expect(reload.session.status).toBe("ended");
    expect(notifications).toEqual(["initializing", "ended"]);
    expect(await ending).toEqual({ providerLogout: "failed" });
    const next = new ApplicationSdk(options());
    await next.restore();
    expect(next.session.status).toBe("unauthenticated");
});
it("unsubscribes and disposes idempotently; a late callback cannot revive or persist a retired session", async () => {
    const sdk = new ApplicationSdk(options());
    const url = await callback(sdk);
    const listener = vi.fn();
    const unsubscribe = sdk.subscribe(listener);
    unsubscribe();
    let release!: (response: Response) => void;
    const held = new Promise<Response>((resolve) => {
        release = resolve;
    });
    service.mockImplementationOnce(() => held);
    const pending = sdk.handleSigninCallback(url);
    const rejected = expect(pending).rejects.toThrow("callback");
    await vi.waitFor(() =>
        expect(
            service.mock.calls.some(([url]) =>
                String(url).endsWith("/oauth2/token"),
            ),
        ).toBe(true),
    );
    sdk.dispose();
    sdk.dispose();
    release(Response.json({ access_token: "late", expires_in: 3600 }));
    await rejected;
    expect(listener).toHaveBeenCalledTimes(1);
    expect(sdk.session.status).toBe("disposed");
    const next = new ApplicationSdk(options());
    await next.restore();
    expect(next.session.status).toBe("unauthenticated");
});
it("does not erase a verified context on transient userinfo failure, but rejects failed identity establishment", async () => {
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const token = sdk.tokenProvider();
    const snapshot = sdk.session;
    service.mockRejectedValueOnce(new Error("temporary outage"));
    await expect(token()).rejects.toThrow("identity");
    expect(sdk.session).toBe(snapshot);
    await expect(token()).resolves.toBe("access");
    const failing = new ApplicationSdk(options("other"));
    const url = await callback(failing);
    service.mockResolvedValueOnce(
        Response.json({ access_token: "unverified", expires_in: 3600 }),
    );
    service.mockResolvedValueOnce(Response.json({ name: "Not an identity" }));
    await expect(failing.handleSigninCallback(url)).rejects.toThrow("callback");
    expect(failing.session.status).not.toBe("authenticated");
});
it("allows concurrent expiry handling in the same context without terminating authentication", async () => {
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const token = sdk.tokenProvider();
    const snapshot = sdk.session;
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 3600000);
    try {
        await expect(Promise.all([token(), token()])).resolves.toEqual([
            "access",
            "access",
        ]);
        expect(sdk.session).toBe(snapshot);
    } finally {
        vi.restoreAllMocks();
    }
});
it("does not accept credentials substituted into a code callback even with a known pending state", async () => {
    const sdk = new ApplicationSdk(options());
    const url = await callback(sdk);
    url.searchParams.delete("code");
    url.searchParams.set("access_token", "substituted");
    url.searchParams.set("expires_in", "3600");
    await expect(sdk.handleSigninCallback(url)).rejects.toThrow("callback");
    expect(sdk.session.status).not.toBe("authenticated");
});
it("retires existing handles when restoration finds credentials missing", async () => {
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const token = sdk.tokenProvider();
    values.clear();
    await sdk.restore();
    expect(sdk.session.status).toBe("ended");
    await expect(token()).rejects.toThrow("ended");
});
it("isolates a late refresh from a new sign-in after disposal", async () => {
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const token = sdk.tokenProvider();
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 3600000);
    let release!: (response: Response) => void;
    const held = new Promise<Response>((resolve) => {
        release = resolve;
    });
    service.mockImplementationOnce(() => held);
    const pending = token();
    const rejected = expect(pending).rejects.toThrow();
    await vi.waitFor(() =>
        expect(
            service.mock.calls.some(
                ([, init]) =>
                    new URLSearchParams(init?.body as string).get(
                        "grant_type",
                    ) === "refresh_token",
            ),
        ).toBe(true),
    );
    sdk.dispose();
    const next = new ApplicationSdk(options());
    await next.handleSigninCallback(await callback(next));
    const snapshot = next.session;
    release(Response.json({ access_token: "obsolete", expires_in: 3600 }));
    await rejected;
    expect(next.session).toBe(snapshot);
    const reload = new ApplicationSdk(options());
    await reload.restore();
    expect(reload.session.status).toBe("authenticated");
    expect([...values.values()].join("")).not.toContain("obsolete");
    vi.restoreAllMocks();
});
it("preserves immutable configuration and emits no sensitive sign-in diagnostics", async () => {
    const configured = options();
    const sdk = new ApplicationSdk(configured);
    configured.client.clientId = "changed";
    const debug = vi.spyOn(console, "debug");
    const assign = vi.fn();
    window.location.assign = assign;
    try {
        await sdk.signin();
        expect(assign).toHaveBeenCalledOnce();
        const url = new URL(assign.mock.calls[0][0]);
        expect(url.searchParams.get("client_id")).toBe("web");
        expect(url.pathname).toBe("/idp/oauth2/authorize");
        expect(
            JSON.parse(url.searchParams.get("registration") ?? "null")
                .application.uri,
        ).toBe("app.test");
        expect(debug).not.toHaveBeenCalled();
    } finally {
        vi.restoreAllMocks();
    }
});
it("retains authentication on userinfo authorization denial but ends it on unauthorized credentials", async () => {
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const token = sdk.tokenProvider();
    const snapshot = sdk.session;
    service.mockResolvedValueOnce(new Response(null, { status: 403 }));
    await expect(token()).rejects.toThrow("identity");
    expect(sdk.session).toBe(snapshot);
    service.mockResolvedValueOnce(new Response(null, { status: 401 }));
    await expect(token()).rejects.toThrow("identity");
    expect(sdk.session.status).toBe("ended");
});
it("cannot navigate or persist a late authorization transaction after disposal during discovery", async () => {
    const sdk = new ApplicationSdk(options());
    let release!: (response: Response) => void;
    const held = new Promise<Response>((resolve) => {
        release = resolve;
    });
    service.mockImplementationOnce(() => held);
    const assign = vi.fn();
    window.location.assign = assign;
    const pending = sdk.signin();
    const rejected = expect(pending).rejects.toThrow("sign-in");
    sdk.dispose();
    release(Response.json(metadata));
    await rejected;
    expect(assign).not.toHaveBeenCalled();
    expect(values.size).toBe(0);
});
it.each([
    "",
    "file:///tmp/private",
    "https://user:secret@files.test/storage",
    "https://files.test/storage?token=private",
    "https://files.test/storage#fragment",
])(
    "rejects malformed/ambiguous service configuration at the owning clients: %s",
    (base) => {
        expect(
            () => new ApplicationSdk({ ...options(), storageBaseUrl: base }),
        ).toThrow();
        expect(
            () => new ApplicationSdk({ ...options(), idpBaseUrl: base }),
        ).toThrow();
    },
);
it("retires visible authentication even when persistent credential removal fails", async () => {
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const listener = vi.fn();
    sdk.subscribe(listener);
    const remove = vi
        .spyOn(window.localStorage, "removeItem")
        .mockImplementation(() => {
            throw new Error("untrusted storage details");
        });
    try {
        await expect(sdk.signout()).rejects.toThrow("clear browser site data");
        expect(sdk.session.status).toBe("ended");
        expect(listener.mock.calls.at(-1)?.[0].status).toBe("ended");
    } finally {
        remove.mockRestore();
    }
});
it("keeps a newer callback authenticated when an older exchange finishes late", async () => {
    const sdk = new ApplicationSdk(options());
    const first = await callback(sdk);
    const second = await callback(sdk);
    let release!: (response: Response) => void;
    const held = new Promise<Response>((resolve) => {
        release = resolve;
    });
    service.mockImplementationOnce(() => held);
    const pending = sdk.handleSigninCallback(first);
    const rejected = expect(pending).rejects.toThrow("callback");
    await vi.waitFor(() =>
        expect(
            service.mock.calls.some(([url]) =>
                String(url).endsWith("/oauth2/token"),
            ),
        ).toBe(true),
    );
    await sdk.handleSigninCallback(second);
    const snapshot = sdk.session;
    release(Response.json({ access_token: "obsolete", expires_in: 3600 }));
    await rejected;
    expect(sdk.session).toBe(snapshot);
    expect(sdk.session.status).toBe("authenticated");
    expect([...values.values()].join("")).not.toContain("obsolete");
});
it("does not let late identity establishment retire a newer successful callback", async () => {
    const sdk = new ApplicationSdk(options());
    const first = await callback(sdk);
    const second = await callback(sdk);
    let release!: (response: Response) => void;
    const held = new Promise<Response>((resolve) => {
        release = resolve;
    });
    service
        .mockResolvedValueOnce(
            Response.json({ access_token: "first", expires_in: 3600 }),
        )
        .mockImplementationOnce(() => held);
    const pending = sdk.handleSigninCallback(first);
    const rejected = expect(pending).rejects.toThrow("callback");
    await vi.waitFor(() =>
        expect(
            service.mock.calls.some(([url]) =>
                String(url).endsWith("/userinfo"),
            ),
        ).toBe(true),
    );
    await sdk.handleSigninCallback(second);
    const snapshot = sdk.session;
    release(Response.json({ sub: "old-user" }));
    await rejected;
    expect(sdk.session).toBe(snapshot);
    expect(sdk.session.status).toBe("authenticated");
});
