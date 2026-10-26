import type { StorageRequest } from "@aicacia/storage-client";
import { ApplicationSdk, type SdkOptions } from "@of/sdk";
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import Page from "../../../routes/(auth)/+page.svelte";
import SignIn from "../../../routes/signin/_SignIn.svelte";
import Browser from "./Browser.svelte";

vi.mock("$lib/common/state/notifications.svelte", () => ({
    notifications: { add: vi.fn() },
}));
vi.mock("$app/navigation", () => ({ goto: vi.fn(), invalidateAll: vi.fn() }));
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
const auth = vi.hoisted(() => ({
    token: "user-token",
    clear: vi.fn(),
    signin: vi.fn(),
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
    getSdk: () =>
        auth.sdk ?? {
            session: {
                status: "authenticated",
                identity: { subject: "user-one" },
                generation: 0,
            },
            subscribe: () => () => {},
            tokenProvider: () => async () => auth.token,
            signin: auth.signin,
            signout: auth.clear,
        },
}));

const id = "01900000-0000-7000-8000-000000000001";
const secret = {
    id,
    name: "First secret",
    secret: "private",
    favorite: false,
    notes: "",
    uris: [
        { uri: "https://first.test", match: "exact" },
        { uri: "https://second.test", match: "host" },
    ],
    fields: [{ name: "Account", value: "kept", type: "hidden" }],
    history: [{ secret: "older", changedAt: "2025-01-01T00:00:00.000Z" }],
    deletedAt: "2025-02-01T00:00:00.000Z",
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
};
const entry = (name: string) => ({ name, hash: "h", size: 1, local: true });
class Endpoint extends EventTarget {
    static OPEN = 1;
    static sockets: Endpoint[] = [];
    static files = new Map<string, Map<string, unknown>>();
    static rejectWrites = false;
    static ambiguousWrites = false;
    static holdWrites = false;
    readyState = 1;
    closed = false;
    requests: StorageRequest[] = [];
    hold = false;
    constructor(readonly url: string) {
        super();
        Endpoint.sockets.push(this);
    }
    send(data: string) {
        const request = JSON.parse(data) as StorageRequest;
        this.requests.push(request);
        if (this.hold) return;
        const fs = new URL(this.url).searchParams.get("filesystem_id");
        if (!fs) throw new Error("Test socket requires a Filesystem ID");
        let files = Endpoint.files.get(fs);
        if (!files) {
            files = new Map(
                fs === "empty" ? [] : [[`secrets/${id}.json`, secret]],
            );
            Endpoint.files.set(fs, files);
        }
        if (
            Endpoint.holdWrites &&
            (request.type === "write" || request.type === "delete")
        )
            return;
        if (!Endpoint.rejectWrites && request.type === "write")
            files.set(
                request.path,
                JSON.parse(
                    new TextDecoder().decode(new Uint8Array(request.content)),
                ),
            );
        if (!Endpoint.rejectWrites && request.type === "delete")
            files.delete(request.path);
        const response =
            request.type === "write" || request.type === "delete"
                ? Endpoint.rejectWrites
                    ? { type: "error", code: "operationFailed" }
                    : Endpoint.ambiguousWrites
                      ? { type: "authenticated" }
                      : request.type === "write"
                        ? { type: "written", entry: entry(request.path) }
                        : { type: "deleted" }
                : request.type === "list"
                  ? {
                        type: "listed",
                        entries:
                            request.path === ""
                                ? files.size === 0
                                    ? []
                                    : [entry("secrets")]
                                : [...files.keys()].map(entry),
                    }
                  : {
                        type: "read",
                        content: [
                            ...new TextEncoder().encode(
                                JSON.stringify(files.get(request.path)),
                            ),
                        ],
                    };
        queueMicrotask(() =>
            this.dispatchEvent(
                new MessageEvent("message", { data: JSON.stringify(response) }),
            ),
        );
    }
    close() {
        this.closed = true;
        this.dispatchEvent(new Event("close"));
    }
}
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
const fetchApi = vi.fn();
beforeEach(() => {
    vi.clearAllMocks();
    auth.token = "user-token";
    auth.sdk = undefined;
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
    Endpoint.sockets = [];
    Endpoint.files = new Map();
    Endpoint.rejectWrites = false;
    Endpoint.ambiguousWrites = false;
    Endpoint.holdWrites = false;
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
    target = document.createElement("div");
    document.body.append(target);
});
afterEach(async () => {
    if (component) await unmount(component);
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

it("discovers, opens by Filesystem ID, reads through the official live query, clears selection on switch, and creates", async () => {
    component = mount(Browser, { target, props: { userId: "user-one" } });
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
            component = mount(Browser, {
                target,
                props: { userId: "user-one" },
            });
            await wait("Duplicate name");
        }
        let release!: (response: Response) => void;
        const held = new Promise<Response>((resolve) => {
            release = resolve;
        });
        const exchange = vi.fn(() => held);
        fetchApi.mockImplementationOnce(exchange);
        if (operation === "creation") click("Create filesystem");
        else
            component = mount(Browser, {
                target,
                props: { userId: "user-one" },
            });
        await vi.waitFor(() => expect(exchange).toHaveBeenCalledTimes(1));
        await unmount(component);
        // A replacement context must not receive the old operation's result.
        component = mount(Browser, { target, props: { userId: "user-two" } });
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

it("saves through the collection, preserves unedited secret fields/history, reopens, creates and deletes", async () => {
    vi.stubGlobal("confirm", () => true);
    component = mount(Browser, { target, props: { userId: "user-one" } });
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    click("First secret");
    input("name", "Edited secret");
    input("secret", "changed");
    input("uri", "https://edited.test");
    click("Save");
    await wait("Saved");
    const saved = Endpoint.files.get("first")?.get(`secrets/${id}.json`);
    expect(saved).toMatchObject({
        ...secret,
        updatedAt: expect.any(String),
        name: "Edited secret",
        secret: "changed",
        uris: [{ uri: "https://edited.test", match: "exact" }, secret.uris[1]],
        history: [
            ...secret.history,
            { secret: "private", changedAt: expect.any(String) },
        ],
    });
    expect((saved as typeof secret).updatedAt).not.toBe(secret.updatedAt);
    click("Open filesystem");
    await wait("Edited secret");
    click("Edited secret");
    expect(target.querySelector<HTMLInputElement>("#secret")?.value).toBe(
        "changed",
    );
    input("notes", "Notes only");
    click("Save");
    await wait("Saved");
    expect(
        (
            Endpoint.files.get("first")?.get(`secrets/${id}.json`) as
                | typeof secret
                | undefined
        )?.history,
    ).toHaveLength(2);
    click("New secret");
    input("name", "New entry");
    input("secret", "new private");
    click("Save");
    await wait("Saved");
    expect(Endpoint.files.get("first")?.size).toBe(2);
    const listItem = [...target.querySelectorAll("li")].find((li) =>
        li.textContent?.includes("New entry"),
    );
    flushSync(() => listItem?.querySelectorAll("button")[1]?.click());
    await wait("Deleted");
    expect(Endpoint.files.get("first")?.size).toBe(1);
    expect(target.querySelector("#secret")).toBeNull();
    click("Open filesystem");
    await wait("Edited secret");
    expect(target.textContent).not.toContain("New entry");
});

it("keeps failed form edits visible, rolls back the list, and validates before writing", async () => {
    component = mount(Browser, { target, props: { userId: "user-one" } });
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    click("First secret");
    input("name", "");
    click("Save");
    await wait("Invalid secret");
    expect(
        Endpoint.sockets[0]?.requests.filter(
            (request) => request.type === "write",
        ),
    ).toHaveLength(0);
    Endpoint.rejectWrites = true;
    input("name", "Rejected edit");
    click("Save");
    await wait("Storage write rejected");
    expect(target.querySelector<HTMLInputElement>("#name")?.value).toBe(
        "Rejected edit",
    );
    expect(target.querySelector("ul")?.textContent).toContain("First secret");
    expect(target.querySelector("ul")?.textContent).not.toContain(
        "Rejected edit",
    );
    expect(target.textContent).not.toContain("Saved");
});

it("shows reconciled ambiguous creates without a success notice or a blind retry draft", async () => {
    component = mount(Browser, { target, props: { userId: "user-one" } });
    await wait("Duplicate name");
    select("empty");
    await wait("No secrets yet");
    click("New secret");
    Endpoint.ambiguousWrites = true;
    input("name", "Possibly saved");
    input("secret", "private");
    click("Save");
    await wait("not acknowledged; authoritative records reloaded");
    expect(target.querySelector("ul")?.textContent).toContain("Possibly saved");
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.textContent).not.toContain("Saved.");
});

it("clears an editor after an ambiguous applied delete without reporting success", async () => {
    vi.stubGlobal("confirm", () => true);
    component = mount(Browser, { target, props: { userId: "user-one" } });
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    click("First secret");
    Endpoint.ambiguousWrites = true;
    click("Delete");
    await wait("not acknowledged; authoritative records reloaded");
    expect(target.textContent).toContain("No secrets yet");
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.textContent).not.toContain("Deleted.");
    click("Open filesystem");
    await wait("No secrets yet");
});

it("refreshes external changes, retires stale editor fields, and manually recovers a disconnected collection", async () => {
    component = mount(Browser, { target, props: { userId: "user-one" } });
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    click("First secret");
    input("name", "Unsaved draft");
    Endpoint.files
        .get("first")
        ?.set(`secrets/${id}.json`, { ...secret, name: "External edit" });
    click("Refresh");
    await wait("Refreshed");
    expect(target.textContent).toContain("External edit");
    expect(target.querySelector("#secret")).toBeNull();
    expect(Endpoint.sockets).toHaveLength(1);
    Endpoint.sockets[0]?.close();
    await wait("verify records before retrying");
    expect(target.querySelector("ul")).toBeNull();
    click("Refresh");
    await wait("Refreshed");
    expect(target.textContent).toContain("External edit");
    expect(target.querySelector('[role="alert"]')).toBeNull();
    expect(Endpoint.sockets).toHaveLength(2);
});

it("qualifies real SDK redirect/callback, discovery/create/open, CRUD, reload, Refresh and local sign-out", async () => {
    vi.stubGlobal("confirm", () => true);
    const storageFetch = fetchApi.getMockImplementation();
    if (!storageFetch) throw new Error("Expected controlled Storage service");
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
        return storageFetch(url, init);
    };
    fetchApi.mockImplementation(identityFetch);
    window.localStorage.clear();
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
        fetch: fetchApi,
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
    // Full-page redirect: a fresh OIDC/SDK instance consumes the persisted transaction.
    auth.sdk = new ApplicationSdk(sdkOptions);
    await auth.sdk.handleSigninCallback(
        new URL(
            `https://app.test/callback?code=once&state=${authorization.searchParams.get("state")}`,
        ),
    );
    expect(auth.setAuthority).toHaveBeenCalledWith("https://idp.test/idp");
    await unmount(component);
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
    Endpoint.files.set("created", new Map());
    click("Create filesystem");
    await wait("No secrets yet");
    expect(Endpoint.sockets.at(-1)?.url).toContain("filesystem_id=created");
    click("New secret");
    input("name", "Workflow secret");
    input("secret", "first password");
    click("Save");
    await wait("Saved");
    input("name", "Workflow edited");
    input("secret", "second password");
    click("Save");
    await wait("Saved");
    const files = Endpoint.files.get("created");
    const saved = [...(files?.values() ?? [])][0] as typeof secret;
    expect(saved).toMatchObject({
        name: "Workflow edited",
        history: [{ secret: "first password", changedAt: expect.any(String) }],
    });
    const old = Endpoint.sockets.at(-1);
    await unmount(component);
    expect(old?.closed).toBe(true);
    // A new mounted app/collection has no in-memory secret state to reuse.
    fetchApi.mockImplementation(async (url: string, init?: RequestInit) =>
        String(url).endsWith("/userinfo") ||
        String(url).endsWith("/.well-known/openid-configuration")
            ? identityFetch(url, init)
            : String(url).endsWith("/oauth2/token")
              ? Response.json({ access_token: "storage-token" })
              : Response.json([
                    { id: "created", name: "Created filesystem" },
                    { id: "empty", name: "Other filesystem" },
                ]),
    );
    auth.sdk = new ApplicationSdk(sdkOptions);
    await auth.sdk.restore();
    component = openApp();
    await wait("Created filesystem");
    select("created");
    await wait("Workflow edited");
    click("Open filesystem");
    await wait("Workflow edited");
    click("Workflow edited");
    expect(target.querySelector<HTMLInputElement>("#secret")?.value).toBe(
        "second password",
    );
    files?.set(`secrets/${saved.id}.json`, {
        ...saved,
        name: "External workflow edit",
    });
    click("Refresh");
    await wait("Refreshed");
    expect(target.textContent).toContain("External workflow edit");
    click("External workflow edit");
    click("Delete");
    await wait("Deleted");
    click("Refresh");
    await wait("Refreshed");
    expect(target.textContent).toContain("No secrets yet");
    expect(files?.size).toBe(0);
    const refreshing = Endpoint.sockets.at(-1);
    if (!refreshing) throw new Error("Expected an open Storage connection");
    refreshing.hold = true;
    click("Refresh");
    await wait("Refreshing secrets");
    expect(
        [...target.querySelectorAll("button")]
            .filter((button) =>
                /^(Refresh|New secret)$/.test(button.textContent?.trim() ?? ""),
            )
            .every((button) => button.disabled),
    ).toBe(true);
    select("empty");
    await wait("No secrets yet");
    expect(refreshing.closed).toBe(true);
    refreshing.dispatchEvent(
        new MessageEvent("message", {
            data: JSON.stringify({
                type: "listed",
                entries: [entry("secrets")],
            }),
        }),
    );
    const signedOut = Endpoint.sockets.at(-1);
    click("Sign out");
    flushSync();
    expect(target.querySelector("select")).toBeNull();
    expect(signedOut?.closed).toBe(true);
    expect(auth.sdk.session.status).toBe("ended");
    const reload = new ApplicationSdk(sdkOptions);
    await reload.restore();
    expect(reload.session.status).toBe("unauthenticated");
    expect(
        fetchApi.mock.calls.some(([url]) =>
            String(url).startsWith("https://storage.test/storage/"),
        ),
    ).toBe(true);
});

it("hides malformed refresh results and recovers only after content repair and explicit retry", async () => {
    component = mount(Browser, { target, props: { userId: "user-one" } });
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    Endpoint.files.get("first")?.set("secrets/broken.json", { invalid: true });
    click("Refresh");
    await wait("Malformed Storage record");
    expect(target.querySelector("ul")).toBeNull();
    expect(target.textContent).not.toContain("No secrets yet");
    expect(Endpoint.sockets).toHaveLength(1);
    Endpoint.files.get("first")?.delete("secrets/broken.json");
    click("Refresh");
    await wait("Refreshed");
    expect(target.textContent).toContain("First secret");
    expect(target.querySelector('[role="alert"]')).toBeNull();
    expect(Endpoint.sockets).toHaveLength(2);
});

it("hides records and prevents blind retries when a write disconnects", async () => {
    component = mount(Browser, { target, props: { userId: "user-one" } });
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    click("First secret");
    Endpoint.holdWrites = true;
    input("name", "Unknown outcome");
    click("Save");
    await vi.waitFor(() =>
        expect(Endpoint.sockets[0]?.requests.at(-1)?.type).toBe("write"),
    );
    Endpoint.sockets[0]?.close();
    await wait("verify records before retrying");
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.querySelector("ul")).toBeNull();
    expect(target.textContent).not.toContain("Saved.");
});

it("does not display a late write completion or error after switching filesystems", async () => {
    component = mount(Browser, { target, props: { userId: "user-one" } });
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
    select("empty");
    await wait("No secrets yet");
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
});

it("ignores a late listing after a rapid Filesystem ID switch", async () => {
    component = mount(Browser, { target, props: { userId: "user-one" } });
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

it("reports malformed records instead of an empty filesystem", async () => {
    const original = Endpoint.prototype.send;
    Endpoint.prototype.send = function (data) {
        if (JSON.parse(data).type !== "read") return original.call(this, data);
        queueMicrotask(() =>
            this.dispatchEvent(
                new MessageEvent("message", {
                    data: JSON.stringify({ type: "read", content: [123] }),
                }),
            ),
        );
    };
    try {
        component = mount(Browser, { target, props: { userId: "user-one" } });
        await wait("Duplicate name");
        select("first");
        await wait("Malformed Storage record");
        expect(target.textContent).not.toContain("No secrets yet");
        expect(Endpoint.sockets[0]?.closed).toBe(true);
    } finally {
        Endpoint.prototype.send = original;
    }
});

it("shows authorization errors without claiming an empty resource", async () => {
    fetchApi.mockResolvedValueOnce(new Response(null, { status: 401 }));
    component = mount(Browser, { target, props: { userId: "user-one" } });
    await wait("Storage token exchange failed: 401");
    expect(target.textContent).not.toContain("No secrets yet");
    expect(Endpoint.sockets).toHaveLength(0);
});

it("clears records and editor during writes when the authenticated User changes and on sign out", async () => {
    const storageFetch = fetchApi.getMockImplementation();
    if (!storageFetch) throw new Error("Expected controlled Storage service");
    let subject = "user-one";
    fetchApi.mockImplementation(
        async (url: URL | RequestInfo, init?: RequestInit) => {
            if (String(url).endsWith("/.well-known/openid-configuration"))
                return Response.json({
                    issuer: "https://installation.test",
                    authorization_endpoint: "https://idp.test/idp/authorize",
                    token_endpoint: "https://idp.test/idp/oauth2/token",
                    userinfo_endpoint: "https://idp.test/idp/userinfo",
                });
            if (String(url).endsWith("/userinfo"))
                return Response.json({ sub: subject });
            if (
                String(url).endsWith("/oauth2/token") &&
                new URLSearchParams(init?.body as string).get("grant_type") ===
                    "authorization_code"
            )
                return Response.json({
                    access_token: "user-token",
                    expires_in: 3600,
                });
            return storageFetch(url, init);
        },
    );
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
        fetch: fetchApi,
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
