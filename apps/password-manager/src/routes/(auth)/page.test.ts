import { ApplicationSdk, type SdkOptions } from "@of/sdk";
import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, beforeEach, expect, it, onTestFinished, vi } from "vitest";
import Page from "./+page.svelte";
import SignIn from "../signin/_SignIn.svelte";
import {
    Endpoint,
    entry,
    id,
    required,
    resetTransport,
} from "$lib/components/secrets/transport.fixture";

vi.mock("$lib/common/state/notifications.svelte", () => ({
    notifications: { add: vi.fn() },
}));
vi.mock("$app/navigation", () => ({ goto: vi.fn(), invalidateAll: vi.fn() }));
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
const auth = vi.hoisted(() => ({
    setAuthority: vi.fn(),
    sdk: undefined as ApplicationSdk | undefined,
    navigate: vi.fn(),
}));
vi.mock("$env/dynamic/public", () => ({
    env: { PUBLIC_STORAGE_BASE_URL: "https://storage.test/storage" },
}));
vi.mock("@aicacia/native-fetch", async (original) => ({
    ...(await original<typeof import("@aicacia/native-fetch")>()),
    openUrl: auth.navigate,
}));
vi.mock("$lib/common/state/sdk", () => ({
    getIdpApiUrl: () => "https://idp.test/idp",
    setIdpApiUrl: auth.setAuthority,
    getSdk: () => {
        if (!auth.sdk)
            throw new Error("Test must authenticate through the real SDK");
        return auth.sdk;
    },
}));

let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
const fetchApi = vi.fn();
const userinfo = vi.fn();
const renewal = vi.fn();
beforeEach(async () => {
    vi.clearAllMocks();
    auth.sdk = undefined;
    userinfo
        .mockReset()
        .mockImplementation(async () => Response.json({ sub: "user-one" }));
    renewal.mockReset().mockImplementation(async () =>
        Response.json({
            access_token: "renewed-user-token",
            refresh_token: "refresh",
            expires_in: 3600,
        }),
    );
    const credentials = new Map<string, string>();
    vi.stubGlobal("localStorage", {
        get length() {
            return credentials.size;
        },
        key: (i: number) => [...credentials.keys()][i] ?? null,
        getItem: (key: string) => credentials.get(key) ?? null,
        setItem: (key: string, value: string) => credentials.set(key, value),
        removeItem: (key: string) => credentials.delete(key),
        clear: () => credentials.clear(),
    });
    resetTransport();
    vi.stubGlobal("WebSocket", Endpoint);
    fetchApi
        .mockReset()
        .mockImplementation(async (url: string, init: RequestInit) =>
            String(url).endsWith("/oauth2/token")
                ? Response.json({ access_token: "storage-token" })
                : init.method === "POST"
                  ? Response.json({ id: "created", name: "New filesystem" })
                  : Response.json([
                        { id: "first", name: "Duplicate name" },
                        { id: "empty", name: "Duplicate name" },
                    ]),
        );
    vi.stubGlobal("fetch", fetchApi);
    auth.sdk = new ApplicationSdk({
        idpBaseUrl: "https://idp.test/idp",
        storageBaseUrl: "https://storage.test/storage",
        disableNativeRequests: true,
        client: {
            clientId: "password-manager-web",
            redirectUri: "https://app.test/callback",
            registration: {
                redirectUris: ["https://app.test/callback"],
                application: { uri: "password-manager.aicacia.com" },
                tokenEndpointAuthMethod: "none",
            },
        },
        fetch: async (url, init) => {
            if (String(url).endsWith("/.well-known/openid-configuration"))
                return Response.json({
                    issuer: "https://installation.test",
                    authorization_endpoint: "https://idp.test/idp/authorize",
                    token_endpoint: "https://idp.test/idp/oauth2/token",
                    userinfo_endpoint: "https://idp.test/idp/userinfo",
                });
            if (String(url).endsWith("/userinfo")) return userinfo(url, init);
            if (String(url).endsWith("/oauth2/client/password-manager-web"))
                return Response.json({
                    client_id: "password-manager-web",
                    application: { uri: "password-manager.aicacia.com" },
                });
            if (
                new URLSearchParams(init?.body as string).get("grant_type") ===
                "refresh_token"
            )
                return renewal(url, init);
            if (
                new URLSearchParams(init?.body as string).get("grant_type") ===
                "authorization_code"
            )
                return Response.json({
                    access_token: "user-token",
                    refresh_token: "refresh",
                    expires_in: 3600,
                });
            return fetchApi(url, init);
        },
    });
    const authorization = await auth.sdk.getAuthorizationUrl();
    await auth.sdk.handleSigninCallback(
        new URL(
            `https://app.test/callback?code=once&state=${authorization.searchParams.get("state")}`,
        ),
    );
    target = document.createElement("div");
    document.body.append(target);
});
afterEach(async () => {
    if (component) await unmount(component);
    auth.sdk?.dispose();
    target.remove();
    vi.unstubAllGlobals();
});
const wait = async (text: string) =>
    vi.waitFor(() => {
        flushSync();
        expect(target.textContent).toContain(text);
    });
function select(id: string) {
    const select = target.querySelector("select");
    if (!select) throw new Error("Filesystem selector is missing");
    flushSync(() => {
        select.value = id;
        select.dispatchEvent(new Event("change"));
    });
}
function mountPage() {
    return mount(Page, {
        target,
        props: {
            params: {},
            data: { userInfo: { sub: "user-one" } },
        },
    });
}

it("discovers, opens by Filesystem ID, reads through the official live query, clears selection on switch, and creates", async () => {
    component = mountPage();
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    Array.from(target.querySelectorAll("button"))
        .find((button) => button.textContent?.includes("First secret"))
        ?.click();
    flushSync();
    expect(target.querySelector<HTMLInputElement>("#secret")?.value).toBe(
        "private",
    );
    expect(
        Array.from(target.querySelectorAll("button"))
            .filter((button) =>
                /Save|Delete|New secret/.test(button.textContent ?? ""),
            )
            .every((button) => !button.disabled),
    ).toBe(true);
    select("empty");
    await wait("No secrets yet");
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.textContent).not.toContain("First secret");
    expect(Endpoint.sockets[0]?.closed).toBe(true);
    expect(Endpoint.sockets[1]?.url).toContain("filesystem_id=empty");
    Array.from(target.querySelectorAll("button"))
        .find((button) => button.textContent?.includes("Create filesystem"))
        ?.click();
    await wait("New filesystem");
    await wait("First secret");
    expect(Endpoint.sockets[2]?.url).toContain("filesystem_id=created");
    expect(
        Endpoint.sockets
            .flatMap((socket) => socket.requests)
            .every(
                (request) => request.type === "read" || request.type === "list",
            ),
    ).toBe(true);
});

it.each(["discovery", "creation"])(
    "unmount cancels %s held in token exchange before any resource request",
    async (operation) => {
        if (operation === "creation") {
            component = mountPage();
            await wait("Duplicate name");
        }
        let release!: (response: Response) => void;
        const held = new Promise<Response>((resolve) => {
            release = resolve;
        });
        const exchange = vi.fn(() => held);
        fetchApi.mockImplementationOnce(exchange);
        if (operation === "creation") click("Create filesystem");
        else component = mountPage();
        await vi.waitFor(() => expect(exchange).toHaveBeenCalledTimes(1));
        expect(
            new URLSearchParams(fetchApi.mock.calls.at(-1)?.[1]?.body).get(
                "grant_type",
            ),
        ).toBe("urn:ietf:params:oauth:grant-type:token-exchange");
        await unmount(component);
        // A replacement context must not receive the old operation's result.
        component = mountPage();
        await wait("Duplicate name");
        const callsBeforeRelease = fetchApi.mock.calls.length;
        release(Response.json({ access_token: "obsolete-storage-token" }));
        await held;
        await new Promise((resolve) => setTimeout(resolve, 20));
        flushSync();
        expect(fetchApi).toHaveBeenCalledTimes(callsBeforeRelease);
        expect(target.textContent).not.toContain("New filesystem");
        expect(Endpoint.sockets).toHaveLength(0);
    },
);

function click(text: string) {
    const button = [...target.querySelectorAll("button")].find(
        (button) => button.textContent?.trim() === text,
    );
    if (!button) throw new Error(`Missing button: ${text}`);
    flushSync(() => button.click());
}
function input(id: string, value: string) {
    const field = target.querySelector<HTMLInputElement>(`#${id}`);
    if (!field) throw new Error(`Missing field: ${id}`);
    flushSync(() => {
        field.value = value;
        field.dispatchEvent(new Event("input", { bubbles: true }));
    });
}

it("wires local draft bindings, Save, Clear, Refresh and confirmed Delete to the workflow", async () => {
    const confirm = vi.fn(() => false);
    vi.stubGlobal("confirm", confirm);
    component = mountPage();
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    click("First secret");
    expect(target.querySelector<HTMLInputElement>("#secret")?.value).toBe(
        "private",
    );
    input("name", "Edited secret");
    input("secret", "changed");
    input("uri", "https://edited.test");
    input("notes", "Edited notes");
    const favorite = required(
        target.querySelector<HTMLInputElement>("#favorite"),
    );
    flushSync(() => favorite.click());
    click("Save");
    await wait("Saved");
    expect(target.querySelector("ul")?.textContent).toContain(
        "★ Edited secret",
    );
    click("Clear");
    expect(target.querySelector("#secret")).toBeNull();
    click("★ Edited secret");
    expect(target.querySelector<HTMLInputElement>("#uri")?.value).toBe(
        "https://edited.test",
    );
    expect(target.querySelector<HTMLTextAreaElement>("#notes")?.value).toBe(
        "Edited notes",
    );
    input("name", "Unsaved draft");
    click("Refresh");
    await wait("Refreshed");
    expect(target.querySelector("#secret")).toBeNull();
    click("Delete");
    expect(confirm).toHaveBeenCalledWith("Delete Edited secret?");
    expect(target.querySelector("ul")?.textContent).toContain("Edited secret");
    confirm.mockReturnValue(true);
    click("Delete");
    await wait("Deleted");
    click("New secret");
    expect(target.querySelector<HTMLInputElement>("#name")?.value).toBe("");
    input("name", "Created entry");
    input("secret", "new private");
    click("Save");
    await wait("Saved");
    expect(target.querySelector("ul")?.textContent).toContain("Created entry");
});

it("keeps the local draft bindings after a rejected Save", async () => {
    component = mountPage();
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    click("First secret");
    Endpoint.rejectWrites = true;
    input("name", "Rejected edit");
    click("Save");
    await wait("Storage write rejected");
    expect(target.querySelector<HTMLInputElement>("#name")?.value).toBe(
        "Rejected edit",
    );
});

it("wires real SDK redirect/callback, filesystem discovery/create/open and local sign-out", async () => {
    const warn = vi.spyOn(console, "warn");
    onTestFinished(() => warn.mockRestore());
    vi.stubGlobal("matchMedia", () => ({
        matches: false,
        addEventListener: () => {},
    }));
    const { load: callbackLoad } = await import("../callback/+page");
    const { load: authenticatedLoad } = await import("./+layout");
    vi.stubGlobal("confirm", () => true);
    const identityFetch = async (
        url: URL | RequestInfo,
        init?: RequestInit,
    ) => {
        const path = String(url);
        if (path.endsWith("/.well-known/openid-configuration"))
            return Response.json({
                issuer: "https://installation.test",
                authorization_endpoint: "https://idp.test/idp/authorize",
                token_endpoint: "https://idp.test/idp/oauth2/token",
                userinfo_endpoint: "https://idp.test/idp/userinfo",
            });
        if (path.endsWith("/userinfo"))
            return Response.json({ sub: "user-one" });
        if (path.endsWith("/oauth2/client/idp-issued-password-manager")) {
            expect(new Headers(init?.headers).get("Authorization")).toBe(
                "Bearer user-token",
            );
            expect(init?.redirect).toBe("error");
            return Response.json({
                client_id: "idp-issued-password-manager",
                application: { uri: "password-manager.aicacia.com" },
            });
        }
        if (
            path.endsWith("/oauth2/token") &&
            new URLSearchParams(init?.body as string).get("grant_type") !==
                "urn:ietf:params:oauth:grant-type:token-exchange"
        )
            return Response.json({
                access_token: "user-token",
                expires_in: 3600,
                refresh_token: "refresh",
            });
        return fetchApi(url, init);
    };
    window.localStorage.clear();
    const sdkOptions: SdkOptions = {
        idpBaseUrl: "https://idp.test/idp",
        storageBaseUrl: "https://storage.test/storage",
        client: {
            redirectUri: "https://app.test/callback",
            registration: {
                redirectUris: ["https://app.test/callback"],
                application: { uri: "password-manager.aicacia.com" },
                tokenEndpointAuthMethod: "none",
            },
        },
        fetch: identityFetch,
        disableNativeRequests: true,
    };
    auth.sdk = new ApplicationSdk(sdkOptions);
    component = mount(SignIn, { target });
    input("remote-url", "https://idp.test/idp");
    target
        .querySelector("form")
        ?.dispatchEvent(new Event("submit", { cancelable: true }));
    await vi.waitFor(() => expect(auth.navigate).toHaveBeenCalledOnce());
    const authorization = auth.navigate.mock.calls[0][0] as URL;
    expect(authorization.searchParams.has("client_id")).toBe(false);
    expect(authorization.searchParams.get("application_id")).toBe(
        "password-manager.aicacia.com",
    );
    expect(
        JSON.parse(authorization.searchParams.get("registration") ?? "null"),
    ).not.toHaveProperty("client_id");
    // Full-page redirect: a fresh OIDC/SDK instance consumes the persisted transaction.
    auth.sdk = new ApplicationSdk(sdkOptions);
    await callbackLoad({
        parent: async () => ({}),
        url: new URL(
            `https://app.test/callback?code=once&client_id=idp-issued-password-manager&iss=https%3A%2F%2Finstallation.test&state=${authorization.searchParams.get("state")}`,
        ),
    } as Parameters<typeof callbackLoad>[0]);
    expect(auth.setAuthority).toHaveBeenCalledWith("https://idp.test/idp");
    expect(auth.sdk.session.identity).toMatchObject({
        application: "password-manager.aicacia.com",
        clientId: "idp-issued-password-manager",
    });
    auth.sdk = new ApplicationSdk(sdkOptions);
    await unmount(component);
    const openApp = async () => {
        const data = await authenticatedLoad({
            parent: async () => ({}),
            url: new URL("https://app.test/"),
        } as Parameters<typeof authenticatedLoad>[0]);
        return mount(Page, { target, props: { params: {}, data: data ?? {} } });
    };
    component = await openApp();
    expect(auth.sdk.session.identity?.clientId).toBe(
        "idp-issued-password-manager",
    );
    await wait("Duplicate name");
    Endpoint.files.set("created", new Map());
    click("Create filesystem");
    await wait("No secrets yet");
    expect(Endpoint.sockets.at(-1)?.url).toContain("filesystem_id=created");
    select("first");
    await wait("First secret");
    click("First secret");
    input("secret", "Unsaved private draft");
    await tick();
    const signedOut = Endpoint.sockets.at(-1);
    click("Sign out");
    flushSync();
    expect(target.querySelector("select")).toBeNull();
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.textContent).not.toContain("First secret");
    expect(signedOut?.closed).toBe(true);
    expect(signedOut?.closes).toBe(1);
    expect(auth.sdk.session.status).toBe("ended");
    const reload = new ApplicationSdk(sdkOptions);
    await reload.restore();
    expect(reload.session.status).toBe("unauthenticated");
    expect(warn.mock.calls.flat().join("\n")).not.toContain("derived_inert");
    expect(
        fetchApi.mock.calls.some(([url]) =>
            String(url).startsWith("https://storage.test/storage/"),
        ),
    ).toBe(true);
});

it.each(["switch", "reopen", "teardown"])(
    "removes records and drafts immediately and ignores late writes on filesystem %s",
    async (operation) => {
        component = mountPage();
        await wait("Duplicate name");
        select("first");
        await wait("First secret");
        click("First secret");
        Endpoint.holdWrites = true;
        input("name", "Pending edit");
        click("Save");
        await wait("Saving");
        expect(
            [...target.querySelectorAll("button")].find(
                (button) => button.textContent?.trim() === "Refresh",
            )?.disabled,
        ).toBe(true);
        await vi.waitFor(() =>
            expect(Endpoint.sockets[0]?.requests.at(-1)?.type).toBe("write"),
        );
        const old = Endpoint.sockets[0];
        if (!old) throw new Error("Expected the original Storage socket");
        if (operation === "switch") select("empty");
        else if (operation === "reopen") click("Open filesystem");
        else {
            await unmount(component);
            expect(target.textContent).toBe("");
            expect(old.closed).toBe(true);
            component = mountPage();
            await wait("Duplicate name");
            select("empty");
        }
        expect(target.querySelector("#secret")).toBeNull();
        expect(target.textContent).not.toContain("First secret");
        await wait(operation === "reopen" ? "First secret" : "No secrets yet");
        old.dispatchEvent(
            new MessageEvent("message", {
                data: JSON.stringify({
                    type: "written",
                    entry: entry(`secrets/${id}.json`),
                }),
            }),
        );
        await new Promise((resolve) => setTimeout(resolve, 0));
        flushSync();
        expect(old.closed).toBe(true);
        expect(target.querySelector("#secret")).toBeNull();
        expect(target.textContent).not.toMatch(
            /Saved|Pending edit|disposed|rejected/,
        );
    },
);

it("ignores a late listing after a rapid Filesystem ID switch", async () => {
    component = mountPage();
    await wait("Duplicate name");
    const original = Endpoint.prototype.send;
    let stale: Endpoint | undefined;
    Endpoint.prototype.send = function (data) {
        if (new URL(this.url).searchParams.get("filesystem_id") === "first") {
            stale = this;
            this.requests.push(JSON.parse(data));
            return;
        }
        original.call(this, data);
    };
    try {
        select("first");
        await wait("Loading secrets");
        await vi.waitFor(() => expect(stale).toBeDefined());
        select("empty");
        await wait("No secrets yet");
        stale?.dispatchEvent(
            new MessageEvent("message", {
                data: JSON.stringify({
                    type: "listed",
                    entries: [entry("secrets")],
                }),
            }),
        );
        flushSync();
        expect(target.textContent).not.toContain("First secret");
        expect(stale?.closed).toBe(true);
        expect(stale?.requests).toHaveLength(1);
    } finally {
        Endpoint.prototype.send = original;
    }
});

it("shows authorization errors without claiming an empty resource", async () => {
    fetchApi.mockResolvedValueOnce(new Response(null, { status: 401 }));
    component = mountPage();
    await wait("Storage token exchange failed: 401");
    expect(target.textContent).not.toContain("No secrets yet");
    expect(Endpoint.sockets).toHaveLength(0);
});

it("offers sign-in recovery after terminal userinfo failure synchronously clears the mounted editor and socket", async () => {
    component = mount(Page, {
        target,
        props: { params: {}, data: { userInfo: { sub: "user-one" } } },
    });
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    click("First secret");
    expect(target.querySelector("#secret")).not.toBeNull();
    userinfo.mockImplementationOnce(
        async () => new Response(null, { status: 401 }),
    );
    await expect(auth.sdk?.storage().listFileSystems()).rejects.toThrow(
        "ended",
    );
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.textContent).not.toContain("First secret");
    expect(Endpoint.sockets[0]?.closed).toBe(true);
    const recovery =
        target.querySelector<HTMLAnchorElement>('a[href="/signin"]');
    expect(recovery?.textContent).toContain("Sign in");
    expect(auth.sdk?.session.status).toBe("ended");
    expect(window.localStorage.length).toBe(0);
});

it.each([200, 401])(
    "keeps the mounted collection/editor and persisted session when stale userinfo (%s) overlaps renewal",
    async (status) => {
        component = mount(Page, {
            target,
            props: { params: {}, data: { userInfo: { sub: "user-one" } } },
        });
        await wait("Duplicate name");
        select("first");
        await wait("First secret");
        click("First secret");
        input("name", "Unsaved renewal draft");
        const editor = target.querySelector("#secret");
        const socket = Endpoint.sockets[0];
        const snapshot = auth.sdk?.session;
        let release!: (response: Response) => void;
        const held = new Promise<Response>((resolve) => {
            release = resolve;
        });
        userinfo.mockImplementationOnce(() => held);
        const access = auth.sdk?.storage();
        if (!access) throw new Error("Expected authenticated Storage");
        const calls = userinfo.mock.calls.length;
        const first = access.listFileSystems();
        await vi.waitFor(() =>
            expect(userinfo.mock.calls.length).toBe(calls + 1),
        );
        const date = vi
            .spyOn(Date, "now")
            .mockReturnValue(Date.now() + 3600000);
        try {
            await access.listFileSystems();
            expect(renewal).toHaveBeenCalledOnce();
            release(
                status === 200
                    ? Response.json({ sub: "user-one" })
                    : new Response(null, { status }),
            );
            await first;
            flushSync();
            expect(auth.sdk?.session).toBe(snapshot);
            expect(target.querySelector("#secret")).toBe(editor);
            expect(target.querySelector<HTMLInputElement>("#name")?.value).toBe(
                "Unsaved renewal draft",
            );
            expect(Endpoint.sockets).toHaveLength(1);
            expect(socket.closed).toBe(false);
            expect(
                window.localStorage.getItem(
                    Array.from({ length: window.localStorage.length }, (_, i) =>
                        window.localStorage.key(i),
                    ).find((key) => key?.includes("oidc.token")) ?? "",
                ),
            ).toContain("renewed-user-token");
            expect(target.textContent).toContain("First secret");
        } finally {
            date.mockRestore();
        }
    },
);

it("clears records and editor during writes when the authenticated User changes and on sign out", async () => {
    let subject = "user-one";
    const identityFetch = async (
        url: URL | RequestInfo,
        init?: RequestInit,
    ) => {
        if (String(url).endsWith("/.well-known/openid-configuration"))
            return Response.json({
                issuer: "https://installation.test",
                authorization_endpoint: "https://idp.test/idp/authorize",
                token_endpoint: "https://idp.test/idp/oauth2/token",
                userinfo_endpoint: "https://idp.test/idp/userinfo",
            });
        if (String(url).endsWith("/userinfo"))
            return Response.json({ sub: subject });
        if (String(url).endsWith("/oauth2/client/password-manager-web"))
            return Response.json({
                client_id: "password-manager-web",
                application: { uri: "password-manager.aicacia.com" },
            });
        if (
            String(url).endsWith("/oauth2/token") &&
            new URLSearchParams(init?.body as string).get("grant_type") ===
                "authorization_code"
        )
            return Response.json({
                access_token: "user-token",
                expires_in: 3600,
            });
        return fetchApi(url, init);
    };
    const sdkOptions: SdkOptions = {
        idpBaseUrl: "https://idp.test/idp",
        storageBaseUrl: "https://storage.test/storage",
        client: {
            clientId: "password-manager-web",
            redirectUri: "https://app.test/callback",
            registration: {
                redirectUris: ["https://app.test/callback"],
                application: { uri: "password-manager.aicacia.com" },
                tokenEndpointAuthMethod: "none",
            },
        },
        fetch: identityFetch,
        disableNativeRequests: true,
    };
    async function authenticate() {
        auth.sdk = new ApplicationSdk(sdkOptions);
        const url = await auth.sdk.getAuthorizationUrl();
        await auth.sdk.handleSigninCallback(
            new URL(
                `https://app.test/callback?code=once&state=${url.searchParams.get("state")}`,
            ),
        );
    }
    await authenticate();
    const openApp = () =>
        mount(Page, {
            target,
            props: {
                params: {},
                get data() {
                    return {
                        userInfo: { sub: auth.sdk?.session.identity?.subject },
                    };
                },
            },
        });
    component = openApp();
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    Array.from(target.querySelectorAll("button"))
        .find((button) => button.textContent?.includes("First secret"))
        ?.click();
    flushSync();
    expect(target.querySelector("#secret")).not.toBeNull();
    Endpoint.holdWrites = true;
    input("name", "Obsolete mutation");
    click("Save");
    await vi.waitFor(() =>
        expect(Endpoint.sockets[0]?.requests.at(-1)?.type).toBe("write"),
    );
    auth.sdk?.dispose();
    expect(target.querySelector("#secret")).toBeNull();
    expect(Endpoint.sockets[0]?.closed).toBe(true);
    await unmount(component);
    subject = "user-two";
    await authenticate();
    component = openApp();
    await wait("Duplicate name");
    expect(target.textContent).not.toContain("First secret");
    expect(target.querySelector("#secret")).toBeNull();
    expect(Endpoint.sockets[0]?.closed).toBe(true);
    Endpoint.sockets[0]?.dispatchEvent(
        new MessageEvent("message", {
            data: JSON.stringify({
                type: "written",
                entry: entry(`secrets/${id}.json`),
            }),
        }),
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    flushSync();
    expect(target.textContent).not.toMatch(/Saved|Obsolete mutation|disposed/);
    select("first");
    await wait("First secret");
    click("First secret");
    input("name", "Signed out mutation");
    click("Save");
    await vi.waitFor(() =>
        expect(Endpoint.sockets[1]?.requests.at(-1)?.type).toBe("write"),
    );
    Array.from(target.querySelectorAll("button"))
        .find((button) => button.textContent?.includes("Sign out"))
        ?.click();
    flushSync();
    expect(target.textContent).not.toContain("First secret");
    expect(Endpoint.sockets[1]?.closed).toBe(true);
    Endpoint.sockets[1]?.dispatchEvent(
        new MessageEvent("message", {
            data: JSON.stringify({
                type: "written",
                entry: entry(`secrets/${id}.json`),
            }),
        }),
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    flushSync();
    expect(target.textContent).not.toMatch(
        /Saved|Signed out mutation|disposed/,
    );
    expect(auth.sdk.session.status).toBe("ended");
    const reload = new ApplicationSdk(sdkOptions);
    await reload.restore();
    expect(reload.session.status).toBe("unauthenticated");
});
