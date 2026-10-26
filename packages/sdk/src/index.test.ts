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
    if (url === "https://files.test/storage/filesystems")
        return Response.json(
            _init?.method === "POST"
                ? { id: "created", name: "Vault" }
                : [{ id: "fs-one", name: "Vault" }],
        );
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
    const storage = sdk.storage();
    const list = () => storage.listFileSystems();
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 3600000);
    try {
        await expect(list()).resolves.toEqual([
            { id: "fs-one", name: "Vault" },
        ]);
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
        await expect(list()).rejects.toThrow("ended");
        expect(sdk.session.status).toBe("ended");
        await expect(list()).rejects.toThrow("ended");
    } finally {
        vi.restoreAllMocks();
    }
});
it("retries stale userinfo after overlapping same-User renewal without retiring persisted authentication", async () => {
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const storage = sdk.storage();
    const snapshot = sdk.session;
    let release!: (response: Response) => void;
    const held = new Promise<Response>((resolve) => {
        release = resolve;
    });
    service.mockImplementationOnce(() => held);
    const first = storage.listFileSystems();
    await vi.waitFor(() =>
        expect(service.mock.results.at(-1)?.value).toBe(held),
    );
    const date = vi.spyOn(Date, "now").mockReturnValue(Date.now() + 3600000);
    try {
        await expect(storage.listFileSystems()).resolves.toEqual([
            { id: "fs-one", name: "Vault" },
        ]);
        release(Response.json({ sub: "alice" }));
        await expect(first).resolves.toEqual([{ id: "fs-one", name: "Vault" }]);
        expect(sdk.session).toBe(snapshot);
        const reload = new ApplicationSdk(options());
        await reload.restore();
        expect(reload.session.identity?.subject).toBe("alice");
    } finally {
        date.mockRestore();
    }
});
it("exchanges only the exact userinfo-verified token and retires a rotated different User before Storage", async () => {
    let issued = 0;
    const exchanged: string[] = [];
    const verified: string[] = [];
    const controlled = async (url: URL | RequestInfo, init?: RequestInit) => {
        const grant = new URLSearchParams(init?.body as string);
        if (String(url).endsWith("/userinfo")) {
            const token =
                new Headers(init?.headers).get("Authorization")?.slice(7) ?? "";
            verified.push(token);
            return Response.json({
                sub: token === "token-4" ? "bob" : "alice",
            });
        }
        if (
            grant.get("grant_type") ===
            "urn:ietf:params:oauth:grant-type:token-exchange"
        ) {
            exchanged.push(grant.get("subject_token") ?? "");
            return Response.json({ access_token: "storage-token" });
        }
        if (String(url).endsWith("/oauth2/token"))
            return Response.json({
                access_token: `token-${++issued}`,
                refresh_token: "refresh",
                expires_in: 1,
            });
        return service(url, init);
    };
    const sdk = new ApplicationSdk({ ...options(), fetch: controlled });
    await sdk.handleSigninCallback(await callback(sdk));
    const storage = sdk.storage();
    await storage.listFileSystems();
    expect(exchanged).toEqual([verified.at(-1)]);
    expect(exchanged).toEqual(["token-3"]);
    await expect(storage.listFileSystems()).rejects.toThrow("ended");
    expect(sdk.session.status).toBe("ended");
    expect(exchanged).toEqual(["token-3"]);
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
    const storage = sdk.storage();
    const list = () => storage.listFileSystems();
    const snapshot = sdk.session;
    service.mockRejectedValueOnce(new Error("temporary outage"));
    await expect(list()).rejects.toThrow("identity");
    expect(sdk.session).toBe(snapshot);
    await expect(list()).resolves.toEqual([{ id: "fs-one", name: "Vault" }]);
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
    const storage = sdk.storage();
    const list = () => storage.listFileSystems();
    const snapshot = sdk.session;
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 3600000);
    try {
        await expect(Promise.all([list(), list()])).resolves.toEqual([
            [{ id: "fs-one", name: "Vault" }],
            [{ id: "fs-one", name: "Vault" }],
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
    const storage = sdk.storage();
    const list = () => storage.listFileSystems();
    values.clear();
    await sdk.restore();
    expect(sdk.session.status).toBe("ended");
    await expect(list()).rejects.toThrow("ended");
});
it("isolates a late refresh from a new sign-in after disposal", async () => {
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const storage = sdk.storage();
    const list = () => storage.listFileSystems();
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 3600000);
    let release!: (response: Response) => void;
    const held = new Promise<Response>((resolve) => {
        release = resolve;
    });
    service.mockImplementationOnce(() => held);
    const pending = list();
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
    const storage = sdk.storage();
    const list = () => storage.listFileSystems();
    const snapshot = sdk.session;
    service.mockResolvedValueOnce(new Response(null, { status: 403 }));
    await expect(list()).rejects.toThrow("identity");
    expect(sdk.session).toBe(snapshot);
    service.mockResolvedValueOnce(new Response(null, { status: 401 }));
    await expect(list()).rejects.toThrow("ended");
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

it("discovers and creates through captured Storage access with explicit service prefixes and action exchange", async () => {
    const sdk = new ApplicationSdk(options());
    expect(() => sdk.storage()).toThrow("Sign in");
    await sdk.handleSigninCallback(await callback(sdk));
    const storage = sdk.storage();
    expect(await storage.listFileSystems()).toEqual([
        { id: "fs-one", name: "Vault" },
    ]);
    expect(await storage.createFileSystem("Vault")).toEqual({
        id: "created",
        name: "Vault",
    });
    const exchanges = service.mock.calls.filter(
        ([, init]) =>
            new URLSearchParams(init?.body as string).get("grant_type") ===
            "urn:ietf:params:oauth:grant-type:token-exchange",
    );
    expect(
        exchanges.map(([, init]) =>
            Object.fromEntries(init?.body as URLSearchParams),
        ),
    ).toEqual([
        expect.objectContaining({
            client_id: "web",
            subject_token: "access",
            resource: "storage:filesystems",
            authorization_details: '[{"type":"storage","actions":["read"]}]',
        }),
        expect.objectContaining({
            authorization_details: '[{"type":"storage","actions":["write"]}]',
        }),
    ]);
    await sdk.signout();
    const calls = service.mock.calls.length;
    await expect(storage.listFileSystems()).rejects.toThrow("ended");
    await expect(storage.createFileSystem("obsolete")).rejects.toThrow("ended");
    await expect(storage.openSocket("fs-one")).rejects.toThrow("ended");
    expect(service).toHaveBeenCalledTimes(calls);
});

class BrowserSocket extends EventTarget {
    static OPEN = 1;
    static instances: BrowserSocket[] = [];
    static connecting = false;
    readyState = BrowserSocket.connecting ? 0 : 1;
    sent: unknown[] = [];
    closes = 0;
    constructor(readonly url: string) {
        super();
        BrowserSocket.instances.push(this);
    }
    send(data: string) {
        this.sent.push(JSON.parse(data));
    }
    close() {
        ++this.closes;
        this.readyState = 3;
        this.dispatchEvent(new Event("close"));
    }
    reply() {
        this.dispatchEvent(
            new MessageEvent("message", {
                data: '{"type":"listed","entries":[]}',
            }),
        );
    }
}
function sockets() {
    BrowserSocket.instances = [];
    BrowserSocket.connecting = false;
    vi.stubGlobal("WebSocket", BrowserSocket);
}
it("isolates lifecycle event target, currentTarget and listener receiver from the raw credential-bearing transport", async () => {
    sockets();
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const connection = await sdk.storage().openSocket("fs-one");
    const endpoint = BrowserSocket.instances[0];
    const observed: unknown[] = [];
    const listener = function (this: unknown, event: Event) {
        observed.push(this, event.target, event.currentTarget);
        expect(event.composedPath()).not.toContain(endpoint);
        expect("data" in event).toBe(false);
        expect("error" in event).toBe(false);
    };
    const removed = vi.fn();
    connection.socket.addEventListener("error", removed);
    connection.socket.removeEventListener("error", removed);
    connection.socket.addEventListener("error", listener, { once: true });
    endpoint.dispatchEvent(
        Object.assign(new Event("error"), { error: endpoint }),
    );
    endpoint.dispatchEvent(new Event("error"));
    expect(observed).toHaveLength(3);
    expect(removed).not.toHaveBeenCalled();
    connection.socket.addEventListener("close", listener);
    await sdk.signout();
    connection.close();
    expect(observed).toHaveLength(6);
    for (const value of observed) {
        expect(value).not.toBe(endpoint);
        expect(value).not.toHaveProperty("send");
        expect(value).not.toHaveProperty("url");
    }
    endpoint.dispatchEvent(new Event("error"));
    expect(observed).toHaveLength(6);
    await expect(
        connection.request({ type: "list", path: "" }),
    ).rejects.toThrow("ended");
    expect(endpoint.sent).toHaveLength(0);
});
it("guards adopted requests and queued work after retirement; only the recipient closes an adopted connection, once", async () => {
    sockets();
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const connection = await sdk.storage().openSocket("fs-one");
    const endpoint = BrowserSocket.instances[0];
    expect("send" in connection.socket).toBe(false);
    expect(endpoint.url).toBe(
        "wss://files.test/storage?access_token=access&filesystem_id=fs-one",
    );
    const exchange = [...service.mock.calls]
        .reverse()
        .find(
            ([, init]) =>
                new URLSearchParams(init?.body as string).get("grant_type") ===
                "urn:ietf:params:oauth:grant-type:token-exchange",
        );
    expect(
        new URLSearchParams(exchange?.[1]?.body as string).get(
            "authorization_details",
        ),
    ).toBe('[{"type":"storage","actions":["read","write"]}]');
    const first = connection.request({ type: "list", path: "" });
    const second = connection.request({ type: "list", path: "secrets" });
    const rejectedFirst = expect(first).rejects.toThrow("ended");
    const rejectedSecond = expect(second).rejects.toThrow("ended");
    await vi.waitFor(() => expect(endpoint.sent).toHaveLength(1));
    await sdk.signout();
    sdk.dispose();
    expect(endpoint.closes).toBe(0);
    endpoint.reply();
    await rejectedFirst;
    await rejectedSecond;
    await expect(
        connection.request({ type: "list", path: "" }),
    ).rejects.toThrow("ended");
    expect(endpoint.sent).toHaveLength(1);
    connection.close();
    connection.close();
    expect(endpoint.closes).toBe(1);
});
it("closes an obsolete successful open before handoff", async () => {
    sockets();
    BrowserSocket.connecting = true;
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const pending = sdk.storage().openSocket("fs-one");
    const rejected = expect(pending).rejects.toThrow("ended");
    await vi.waitFor(() => expect(BrowserSocket.instances).toHaveLength(1));
    const endpoint = BrowserSocket.instances[0];
    endpoint.readyState = 1;
    endpoint.dispatchEvent(new Event("open"));
    sdk.dispose();
    await rejected;
    expect(endpoint.closes).toBe(1);
    expect(endpoint.sent).toHaveLength(0);
});

it("disposal cancels an untransferred opening socket even when the server never opens", async () => {
    sockets();
    BrowserSocket.connecting = true;
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const pending = sdk.storage().openSocket("fs-one");
    const rejected = expect(pending).rejects.toThrow();
    await vi.waitFor(() => expect(BrowserSocket.instances).toHaveLength(1));
    sdk.dispose();
    await vi.waitFor(() => expect(BrowserSocket.instances[0].closes).toBe(1));
    await rejected;
});

it.each(["list", "create"])(
    "rejects an obsolete %s completion without affecting a replacement context",
    async (operation) => {
        const controlled = vi.fn(service.getMockImplementation());
        const sdk = new ApplicationSdk({ ...options(), fetch: controlled });
        await sdk.handleSigninCallback(await callback(sdk));
        const access = sdk.storage();
        let release!: (response: Response) => void;
        const held = new Promise<Response>((resolve) => {
            release = resolve;
        });
        controlled.mockImplementation((url, init) =>
            String(url).endsWith("/storage/filesystems")
                ? held
                : service(url, init),
        );
        const pending =
            operation === "list"
                ? access.listFileSystems()
                : access.createFileSystem("Vault");
        const rejected = expect(pending).rejects.toThrow("ended");
        await vi.waitFor(() =>
            expect(
                controlled.mock.calls.some(([url]) =>
                    String(url).endsWith("/storage/filesystems"),
                ),
            ).toBe(true),
        );
        sdk.dispose();
        const next = new ApplicationSdk(options());
        await next.handleSigninCallback(await callback(next));
        const snapshot = next.session;
        release(
            Response.json(
                operation === "list"
                    ? [{ id: "obsolete", name: "Old" }]
                    : { id: "obsolete", name: "Old" },
            ),
        );
        await rejected;
        expect(next.session).toBe(snapshot);
        expect(await next.storage().listFileSystems()).toEqual([
            { id: "fs-one", name: "Vault" },
        ]);
    },
);
it.each([401, 403, "offline"])(
    "does not end authentication on Storage denial/transient failure: %s",
    async (failure) => {
        const controlled = vi.fn(service.getMockImplementation());
        const sdk = new ApplicationSdk({ ...options(), fetch: controlled });
        await sdk.handleSigninCallback(await callback(sdk));
        const snapshot = sdk.session;
        const access = sdk.storage();
        controlled.mockImplementation((url, init) => {
            if (!String(url).endsWith("/storage/filesystems"))
                return service(url, init);
            if (failure === "offline") throw new Error("Network unavailable");
            return Promise.resolve(
                new Response(null, { status: Number(failure) }),
            );
        });
        await expect(access.listFileSystems()).rejects.toThrow(
            failure === "offline"
                ? "request failed"
                : `Filesystem list failed: ${failure}`,
        );
        expect(sdk.session).toBe(snapshot);
        controlled.mockImplementation(service.getMockImplementation());
        await expect(access.listFileSystems()).resolves.toEqual([
            { id: "fs-one", name: "Vault" },
        ]);
    },
);
it("caller cancellation closes a failed open, and closing an adopted handle rejects new requests without transport work", async () => {
    sockets();
    BrowserSocket.connecting = true;
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const access = sdk.storage();
    const controller = new AbortController();
    const pending = access.openSocket("fs-one", controller.signal);
    const rejected = expect(pending).rejects.toThrow("cancelled");
    await vi.waitFor(() => expect(BrowserSocket.instances).toHaveLength(1));
    controller.abort();
    await rejected;
    expect(BrowserSocket.instances[0].closes).toBe(1);
    BrowserSocket.connecting = false;
    const adopted = await access.openSocket("fs-one");
    adopted.close();
    adopted.close();
    await expect(adopted.request({ type: "list", path: "" })).rejects.toThrow(
        "closed",
    );
    expect(BrowserSocket.instances[1].closes).toBe(1);
    expect(BrowserSocket.instances[1].sent).toEqual([]);
});

it("rejects newly requested work immediately after retirement even behind an outstanding request", async () => {
    sockets();
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const connection = await sdk.storage().openSocket("fs-one");
    const pending = connection.request({ type: "list", path: "" });
    const rejected = expect(pending).rejects.toThrow("ended");
    await vi.waitFor(() =>
        expect(BrowserSocket.instances[0].sent).toHaveLength(1),
    );
    await sdk.signout();
    let denied = false;
    const next = connection
        .request({ type: "list", path: "secrets" })
        .catch(() => {
            denied = true;
        });
    try {
        await vi.waitFor(() => expect(denied).toBe(true));
    } finally {
        connection.close();
        await rejected;
        await next;
    }
    expect(BrowserSocket.instances[0].sent).toHaveLength(1);
});

it.each(["list", "create", "open"])(
    "cannot send obsolete %s work held in token exchange into a replacement context",
    async (operation) => {
        sockets();
        const controlled = vi.fn(service.getMockImplementation());
        const sdk = new ApplicationSdk({ ...options(), fetch: controlled });
        await sdk.handleSigninCallback(await callback(sdk));
        const access = sdk.storage();
        let release!: (response: Response) => void;
        let exchanging = false;
        const held = new Promise<Response>((resolve) => {
            release = resolve;
        });
        controlled.mockImplementation((url, init) => {
            if (
                new URLSearchParams(init?.body as string).get("grant_type") ===
                "urn:ietf:params:oauth:grant-type:token-exchange"
            ) {
                exchanging = true;
                return held;
            }
            return service(url, init);
        });
        const pending =
            operation === "list"
                ? access.listFileSystems()
                : operation === "create"
                  ? access.createFileSystem("Vault")
                  : access.openSocket("fs-one");
        const rejected = expect(pending).rejects.toThrow("ended");
        await vi.waitFor(() => expect(exchanging).toBe(true));
        sdk.dispose();
        const next = new ApplicationSdk(options());
        await next.handleSigninCallback(await callback(next));
        const snapshot = next.session;
        const requests = controlled.mock.calls.length;
        release(Response.json({ access_token: "obsolete-storage-token" }));
        await rejected;
        expect(controlled).toHaveBeenCalledTimes(requests);
        expect(BrowserSocket.instances).toHaveLength(0);
        expect(next.session).toBe(snapshot);
    },
);
it("replaces callback generations without letting retained Storage access follow the new User", async () => {
    sockets();
    const sdk = new ApplicationSdk(options());
    await sdk.handleSigninCallback(await callback(sdk));
    const old = sdk.storage();
    const connection = await old.openSocket("fs-one");
    const next = await callback(sdk);
    subject = "bob";
    await sdk.handleSigninCallback(next);
    const calls = service.mock.calls.length;
    await expect(old.listFileSystems()).rejects.toThrow("ended");
    await expect(
        connection.request({ type: "list", path: "" }),
    ).rejects.toThrow("ended");
    expect(service).toHaveBeenCalledTimes(calls);
    expect(sdk.session.identity?.subject).toBe("bob");
    connection.close();
    await expect(sdk.storage().listFileSystems()).resolves.toEqual([
        { id: "fs-one", name: "Vault" },
    ]);
});
