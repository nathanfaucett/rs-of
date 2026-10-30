import { ApplicationSdk, type SdkOptions } from "@of/sdk";
import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, beforeEach, expect, it, onTestFinished, vi } from "vitest";
import Page from "./+page.svelte";
import SignIn from "../signin/_SignIn.svelte";
import {
    SqlTransport,
    databaseId,
    id,
    required,
    secret,
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
let transport: SqlTransport;
let sdkOptions: SdkOptions;
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
    transport = new SqlTransport();
    fetchApi
        .mockReset()
        .mockImplementation((url, init) => transport.fetch(url, init));
    vi.stubGlobal("fetch", fetchApi);
    sdkOptions = {
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
    };
    auth.sdk = new ApplicationSdk(sdkOptions);
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
function mountPage(subject = "user-one") {
    return mount(Page, {
        target,
        props: {
            params: {},
            data: { userInfo: { sub: subject } },
        },
    });
}

it("automatically provisions a ready empty SQL vault without resource controls", async () => {
    expect(transport.requests).toHaveLength(0);
    component = mountPage();
    await wait("No secrets yet");
    expect(target.querySelector("select")).toBeNull();
    expect(target.textContent).not.toMatch(
        /filesystem|Open database|Create database/,
    );
    const ensure = required(
        transport.requests.find(({ url }) => url.endsWith("/ensure")),
    );
    expect(JSON.parse(ensure.init.body as string)).toEqual({
        creationKey: "password-manager:default",
        name: "Password Manager",
    });
    expect(transport.initialized).toBe(true);
});

function click(text: string) {
    const button = [...target.querySelectorAll("button")].find(
        (button) => button.textContent?.trim() === text,
    );
    if (!button) throw new Error(`Missing button: ${text}`);
    flushSync(() => button.click());
}
function input(id: string, value: string) {
    const field = required(target.querySelector<HTMLInputElement>(`#${id}`));
    flushSync(() => {
        field.value = value;
        field.dispatchEvent(new Event("input", { bubbles: true }));
    });
}
function list() {
    return target.querySelector("ul")?.textContent ?? "";
}
const mutations = () =>
    transport.requests.filter(({ request }) =>
        request?.statements.some(({ sql }) =>
            /^(INSERT INTO secrets|UPDATE secrets|DELETE FROM secrets)/.test(
                sql,
            ),
        ),
    );

it("validates drafts and persists CRUD, metadata and password history through the real collection", async () => {
    transport.seed();
    vi.stubGlobal("confirm", () => true);
    component = mountPage();
    await wait("First secret");
    click("First secret");
    input("name", "");
    click("Save");
    await wait("Invalid secret");
    expect(mutations()).toHaveLength(0);
    input("name", "Renamed");
    input("secret", "changed");
    input("uri", "https://edited.test");
    flushSync(() =>
        required(target.querySelector<HTMLInputElement>("#favorite")).click(),
    );
    click("Save");
    await wait("Saved. Authoritative");
    expect(list()).toContain("★ Renamed");
    expect(transport.records.get(id)).toMatchObject({
        ...secret,
        name: "Renamed",
        secret: "changed",
        favorite: true,
        uris: [{ uri: "https://edited.test", match: "exact" }, secret.uris[1]],
        updatedAt: expect.any(String),
        history: [
            ...secret.history,
            { secret: "private", changedAt: expect.any(String) },
        ],
    });
    input("uri", "");
    input("notes", "New notes");
    click("Save");
    await wait("Saved. Authoritative");
    expect(transport.records.get(id)).toMatchObject({
        uris: [secret.uris[1]],
        notes: "New notes",
        history: expect.any(Array),
    });
    expect((transport.records.get(id) as typeof secret).history).toHaveLength(
        2,
    );
    click("Clear");
    click("New secret");
    input("name", "Created");
    input("secret", "test-only");
    click("Save");
    await wait("Saved. Authoritative");
    const created = required(
        [...transport.records.values()].find(
            (record) => (record as typeof secret).name === "Created",
        ),
    ) as typeof secret;
    expect(created.id).toMatch(
        /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
    expect(created).toMatchObject({
        fields: [],
        uris: [],
        history: [],
        favorite: false,
    });
    click("Clear");
    const createdRow = [...target.querySelectorAll("li")].find((row) =>
        row.textContent?.includes("Created"),
    );
    flushSync(() =>
        required(createdRow?.querySelectorAll("button")[1]).click(),
    );
    await wait("Deleted. Authoritative");
    expect(list()).not.toContain("Created");
    expect(transport.records.has(created.id)).toBe(false);
    const persistedBrowserState = Array.from(
        { length: localStorage.length },
        (_, i) => localStorage.getItem(localStorage.key(i) ?? ""),
    ).join();
    expect(persistedBrowserState).not.toMatch(
        /First secret|Renamed|New notes|test-only|older/,
    );
});

it("reopens persisted records after a fresh SDK restoration without resetting schema or data", async () => {
    component = mountPage();
    await wait("No secrets yet");
    click("New secret");
    input("name", "Across reload");
    input("secret", "test-only");
    click("Save");
    await wait("Saved. Authoritative");
    await unmount(component);
    const previousRuntime = auth.sdk;
    onTestFinished(() => previousRuntime?.dispose());
    auth.sdk = new ApplicationSdk(sdkOptions);
    await auth.sdk.restore();
    const before = transport.requests.length;
    component = mountPage();
    await wait("Across reload");
    expect(
        transport.requests
            .slice(before)
            .filter(({ request }) =>
                request?.statements.some(({ sql }) =>
                    /^(CREATE|INSERT)/.test(sql),
                ),
            ),
    ).toHaveLength(0);
    expect(transport.records.size).toBe(1);
    expect(
        transport.requests.filter(({ url }) => url.endsWith("/ensure")),
    ).toHaveLength(2);
});

it.each([401, 403, 404, 409, 410, 503])(
    "fails closed on provisioning status %s; explicit retry never chooses another identity",
    async (status) => {
        transport.seed();
        transport.ensureStatus = status;
        component = mountPage();
        await wait(
            status === 410
                ? "Database deleted"
                : status === 409
                  ? "fenced"
                  : status === 401 || status === 403
                    ? "Storage access denied"
                    : "Database unavailable",
        );
        expect(target.textContent).not.toContain("No secrets yet");
        expect(target.querySelector("ul")).toBeNull();
        expect(auth.sdk?.session.status).toBe("authenticated");
        expect(transport.records.size).toBe(1);
        expect(transport.requests).toHaveLength(1);
        if (status !== 410) transport.ensureStatus = 200;
        click("Retry preparation");
        if (status !== 410) await wait("First secret");
        else await vi.waitFor(() => expect(transport.requests).toHaveLength(2));
        const ensures = transport.requests.filter(({ url }) =>
            url.endsWith("/ensure"),
        );
        expect(ensures).toHaveLength(2);
        expect(ensures[0].init.body).toBe(ensures[1].init.body);
        expect(transport.records.size).toBe(1);
    },
);

it("retries a lost ensure response explicitly using the same creation key", async () => {
    let lost = true;
    fetchApi.mockImplementation(async (url, init) => {
        if (String(url).endsWith("/ensure") && lost) {
            lost = false;
            await transport.fetch(url, init);
            throw new Error("PRIVATE unavailable body");
        }
        return transport.fetch(url, init);
    });
    component = mountPage();
    await wait("Storage request failed");
    expect(target.textContent).not.toContain("PRIVATE");
    click("Retry preparation");
    await wait("No secrets yet");
    const ensures = transport.requests.filter(({ url }) =>
        url.endsWith("/ensure"),
    );
    expect(ensures).toHaveLength(2);
    expect(ensures[0].init.body).toBe(ensures[1].init.body);
});

it.each(["2", "", "01"])(
    "rejects incompatible/malformed schema version %s without resetting it",
    async (version) => {
        transport.seed();
        transport.version = version;
        component = mountPage();
        await wait("Incompatible or malformed database schema");
        expect(transport.version).toBe(version);
        expect(transport.records.size).toBe(1);
        expect(
            transport.requests.some(({ request }) =>
                request?.statements.some(({ sql }) =>
                    /^(CREATE|INSERT|UPDATE|DELETE)/.test(sql),
                ),
            ),
        ).toBe(false);
        expect(target.textContent).not.toContain("No secrets yet");
    },
);

it("refuses a partially missing schema rather than initializing over existing content", async () => {
    transport.seed();
    transport.tables = ["secrets"];
    component = mountPage();
    await wait("Malformed database schema");
    expect(transport.requests).toHaveLength(2);
    expect(transport.records.size).toBe(1);
});

it("Refresh reloads external additions, edits and deletions in favorite/name order and closes stale drafts", async () => {
    transport.seed();
    component = mountPage();
    await wait("First secret");
    click("First secret");
    input("name", "Unsaved stale draft");
    transport.records.set(id, { ...secret, name: "Zulu", favorite: true });
    const alpha = "01900000-0000-7000-8000-000000000002";
    const beta = "01900000-0000-7000-8000-000000000003";
    transport.records.set(alpha, { ...secret, id: alpha, name: "Alpha" });
    transport.records.set(beta, {
        ...secret,
        id: beta,
        name: "Beta",
        favorite: true,
    });
    expect(list()).toContain("First secret");
    click("Refresh");
    await wait("Refreshed. Authoritative");
    expect(
        [...target.querySelectorAll("li")].map(
            (row) => row.querySelector("button")?.textContent,
        ),
    ).toEqual(["★ Beta", "★ Zulu", "Alpha"]);
    expect(target.querySelector("#name")).toBeNull();
    transport.records.delete(id);
    click("Refresh");
    await wait("Refreshed. Authoritative");
    expect(list()).not.toContain("Zulu");
    expect(
        transport.requests.filter(({ url }) => url.endsWith("/ensure")),
    ).toHaveLength(1);
});

it.each(["load", "refresh"])(
    "never publishes a partial or empty malformed %s and recovers only on explicit Refresh",
    async (operation) => {
        transport.seed();
        if (operation === "load") transport.malformed = true;
        component = mountPage();
        if (operation === "refresh") {
            await wait("First secret");
            click("First secret");
            transport.malformed = true;
            click("Refresh");
        }
        await wait("Malformed Storage record");
        expect(target.textContent).not.toMatch(
            /First secret|No secrets yet|broken private JSON/,
        );
        expect(target.querySelector("#secret")).toBeNull();
        const requests = transport.requests.length;
        transport.malformed = false;
        await tick();
        expect(transport.requests).toHaveLength(requests);
        click("Refresh");
        await wait("First secret");
    },
);

it.each(["create", "update", "delete"])(
    "reconciles rejected %s without success and retains safe drafts",
    async (operation) => {
        transport.seed();
        vi.stubGlobal("confirm", () => true);
        component = mountPage();
        await wait("First secret");
        if (operation === "create") {
            click("New secret");
            input("secret", "test-only");
        } else click("First secret");
        input("name", "Rejected edit");
        transport.writeStatus = 400;
        click(operation === "delete" ? "Delete" : "Save");
        await wait("Storage write rejected");
        expect(list()).toContain("First secret");
        expect(target.querySelector<HTMLInputElement>("#name")?.value).toBe(
            "Rejected edit",
        );
        expect(target.textContent).not.toMatch(/Saved\.|Deleted\./);
        expect(transport.records.size).toBe(1);
        expect(mutations()).toHaveLength(1);
    },
);

it.each(["create", "update", "delete"])(
    "reconciles uncertain %s without replay or false success and closes its draft",
    async (operation) => {
        transport.seed();
        transport.uncertain = true;
        vi.stubGlobal("confirm", () => true);
        component = mountPage();
        await wait("First secret");
        if (operation === "create") {
            click("New secret");
            input("secret", "test-only");
        } else click("First secret");
        input("name", "Possibly persisted");
        click(operation === "delete" ? "Delete" : "Save");
        await wait("not acknowledged; authoritative records reloaded");
        expect(target.querySelector("#secret")).toBeNull();
        expect(target.textContent).not.toMatch(
            /Saved\.|Deleted\.|PRIVATE BODY/,
        );
        expect(mutations()).toHaveLength(1);
        if (operation === "delete")
            expect(list()).not.toContain("First secret");
        else expect(list()).toContain("Possibly persisted");
    },
);

it("failed post-write reconciliation hides all records and drafts until explicit recovery", async () => {
    transport.seed();
    component = mountPage();
    await wait("First secret");
    click("First secret");
    transport.malformed = true;
    input("name", "Applied but untrusted");
    click("Save");
    await wait("Refresh and verify records before retrying");
    expect(target.querySelector("ul")).toBeNull();
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.textContent).not.toMatch(/Saved\.|No secrets yet/);
    transport.malformed = false;
    click("Refresh");
    await wait("Applied but untrusted");
    expect(mutations()).toHaveLength(1);
});

function holdRequest(matches: (url: string, init: RequestInit) => boolean) {
    const heldTransport = transport;
    let release!: (response: Response) => void;
    let captured: [URL | RequestInfo, RequestInit] | undefined;
    const pending = new Promise<Response>((resolve) => {
        release = resolve;
    });
    fetchApi.mockImplementation((url, init) => {
        if (!captured && matches(String(url), init)) {
            captured = [url, init];
            return pending;
        }
        return transport.fetch(url, init);
    });
    return {
        async started() {
            await vi.waitFor(() => expect(captured).toBeDefined());
        },
        get signal() {
            return captured?.[1].signal;
        },
        async release() {
            const [url, init] = required(captured);
            release(await heldTransport.fetch(url, init));
            await pending;
            await new Promise((resolve) => setTimeout(resolve, 0));
            flushSync();
        },
    };
}
const isSnapshot = (url: string, init: RequestInit) =>
    url.endsWith("/sql") &&
    String(init.body).includes("SELECT id, content FROM secrets");

it.each(["save", "delete"])(
    "disables conflicting actions and reports no success while %s or its reconciliation is pending",
    async (operation) => {
        transport.seed();
        vi.stubGlobal("confirm", () => true);
        component = mountPage();
        await wait("First secret");
        click("First secret");
        const held = holdRequest(
            (url, init) =>
                url.endsWith("/sql") &&
                /UPDATE secrets|DELETE FROM secrets/.test(String(init.body)),
        );
        input("name", "Pending edit");
        click(operation === "save" ? "Save" : "Delete");
        await held.started();
        expect(
            [...target.querySelectorAll("button")]
                .filter((button) =>
                    /Refresh|Save|Delete|Clear|New secret/.test(
                        button.textContent ?? "",
                    ),
                )
                .every((button) => button.disabled),
        ).toBe(true);
        expect(target.textContent).not.toMatch(/Saved\.|Deleted\./);
        if (operation === "delete")
            expect(list()).not.toContain("First secret");
        else expect(list()).toContain("Pending edit");
        const reconciliation = holdRequest(isSnapshot);
        await held.release();
        await reconciliation.started();
        expect(target.textContent).not.toMatch(/Saved\.|Deleted\./);
        await reconciliation.release();
        await wait(
            operation === "save"
                ? "Saved. Authoritative"
                : "Deleted. Authoritative",
        );
    },
);

it.each(["provisioning", "schema", "load", "refresh", "write"])(
    "retirement during %s immediately clears secrets and suppresses late results",
    async (operation) => {
        transport.seed();
        if (operation === "refresh" || operation === "write") {
            component = mountPage();
            await wait("First secret");
            click("First secret");
            input("name", "Private draft");
        }
        const held = holdRequest((url, init) =>
            operation === "provisioning"
                ? url.endsWith("/ensure")
                : operation === "schema"
                  ? String(init.body).includes("SELECT id, version")
                  : operation === "write"
                    ? String(init.body).includes("UPDATE secrets")
                    : isSnapshot(url, init),
        );
        if (operation === "refresh") click("Refresh");
        else if (operation === "write") click("Save");
        else component = mountPage();
        await held.started();
        if (operation === "provisioning" || operation === "schema")
            expect(target.textContent).toContain("Preparing secrets database");
        if (operation === "load")
            expect(target.textContent).toContain("Loading secrets");
        const signingOut = auth.sdk?.signout();
        expect(target.querySelector("#secret")).toBeNull();
        expect(target.textContent).not.toContain("First secret");
        expect(target.textContent).toContain("Sign in again");
        expect(held.signal?.aborted).toBe(true);
        await held.release();
        await signingOut;
        expect(target.textContent).not.toMatch(
            /First secret|Private draft|Saved\.|Refreshed\.|No secrets yet/,
        );
        const reload = new ApplicationSdk(sdkOptions);
        await reload.restore();
        expect(reload.session.status).toBe("unauthenticated");
        reload.dispose();
    },
);

it.each(["provisioning", "load", "refresh", "write"])(
    "route teardown during %s cannot populate a replacement mount with an obsolete outcome",
    async (operation) => {
        transport.seed();
        if (operation === "refresh" || operation === "write") {
            component = mountPage();
            await wait("First secret");
            click("First secret");
        }
        const held = holdRequest((url, init) =>
            operation === "provisioning"
                ? url.endsWith("/ensure")
                : operation === "write"
                  ? String(init.body).includes("UPDATE secrets")
                  : isSnapshot(url, init),
        );
        if (operation === "refresh") click("Refresh");
        else if (operation === "write") {
            input("name", "Obsolete write");
            click("Save");
        } else component = mountPage();
        await held.started();
        await unmount(component);
        expect(target.textContent).toBe("");
        expect(held.signal?.aborted).toBe(true);
        component = mountPage();
        await wait("First secret");
        await held.release();
        expect(list()).toContain("First secret");
        expect(target.textContent).not.toMatch(
            /Obsolete write|Saved\.|Refreshed\./,
        );
        expect(target.querySelector("#secret")).toBeNull();
    },
);

it.each(["identity", "terminal", "dispose"])(
    "synchronously clears a mounted editor on SDK %s retirement",
    async (reason) => {
        transport.seed();
        component = mountPage();
        await wait("First secret");
        click("First secret");
        if (reason === "dispose") auth.sdk?.dispose();
        else {
            userinfo.mockImplementationOnce(async () =>
                reason === "identity"
                    ? Response.json({ sub: "another-user" })
                    : new Response(null, { status: 401 }),
            );
            await expect(auth.sdk?.storage().listDatabases()).rejects.toThrow(
                "ended",
            );
        }
        expect(target.querySelector("#secret")).toBeNull();
        expect(target.textContent).not.toContain("First secret");
        expect(target.textContent).toContain("Sign in again");
    },
);

it.each([200, 401])(
    "same-identity renewal keeps the exact editor during stale userinfo status %s",
    async (status) => {
        transport.seed();
        component = mountPage();
        await wait("First secret");
        click("First secret");
        input("name", "Unsaved renewal draft");
        const editor = target.querySelector("#secret");
        const snapshot = auth.sdk?.session;
        let release!: (response: Response) => void;
        const held = new Promise<Response>((resolve) => {
            release = resolve;
        });
        userinfo.mockImplementationOnce(() => held);
        const access = required(auth.sdk).storage();
        const calls = userinfo.mock.calls.length;
        const first = access.getDatabase(databaseId);
        // These explicit metadata reads are not application provisioning.
        fetchApi.mockImplementation((url, init) =>
            String(url).endsWith(`/databases/${databaseId}`)
                ? Response.json({ id: databaseId })
                : transport.fetch(url, init),
        );
        await vi.waitFor(() =>
            expect(userinfo.mock.calls.length).toBe(calls + 1),
        );
        const date = vi
            .spyOn(Date, "now")
            .mockReturnValue(Date.now() + 3600000);
        try {
            await access.getDatabase(databaseId);
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
            expect(
                transport.requests.filter(({ url }) => url.endsWith("/ensure")),
            ).toHaveLength(1);
        } finally {
            date.mockRestore();
        }
    },
);

it("runs app sign-in navigation and fresh-runtime callback/route restoration into an automatically opened vault", async () => {
    vi.stubGlobal("matchMedia", () => ({
        matches: false,
        addEventListener: () => {},
    }));
    auth.sdk?.dispose();
    const baseFetch = required(sdkOptions.fetch);
    sdkOptions = {
        ...sdkOptions,
        client: { ...sdkOptions.client, clientId: undefined },
        fetch: async (url, init) =>
            String(url).endsWith("/oauth2/client/idp-issued-password-manager")
                ? Response.json({
                      client_id: "idp-issued-password-manager",
                      application: { uri: "password-manager.aicacia.com" },
                  })
                : baseFetch(url, init),
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
    const signinRuntime = auth.sdk;
    onTestFinished(() => signinRuntime?.dispose());
    auth.sdk = new ApplicationSdk(sdkOptions);
    const { load: callbackLoad } = await import("../callback/+page");
    await callbackLoad({
        parent: async () => ({}),
        url: new URL(
            `https://app.test/callback?code=once&client_id=idp-issued-password-manager&iss=https%3A%2F%2Finstallation.test&state=${authorization.searchParams.get("state")}`,
        ),
    } as Parameters<typeof callbackLoad>[0]);
    expect(auth.sdk.session.identity?.clientId).toBe(
        "idp-issued-password-manager",
    );
    expect(transport.requests).toHaveLength(0);
    await unmount(component);
    const callbackRuntime = auth.sdk;
    onTestFinished(() => callbackRuntime.dispose());
    auth.sdk = new ApplicationSdk(sdkOptions);
    const { load: authenticatedLoad } = await import("./+layout");
    const data = await authenticatedLoad({
        parent: async () => ({}),
        url: new URL("https://app.test/"),
    } as Parameters<typeof authenticatedLoad>[0]);
    component = mount(Page, {
        target,
        props: { params: {}, data: data ?? {} },
    });
    await wait("No secrets yet");
    click("New secret");
    input("name", "Signed-in test");
    input("secret", "test-only");
    click("Save");
    await wait("Saved. Authoritative");
    click("Sign out");
    expect(target.querySelector("#secret")).toBeNull();
    expect(target.textContent).not.toContain("Signed-in test");
    await wait("Sign in again");
});

it("teardown cancels provisioning held in token exchange before sending any resource request", async () => {
    const held = holdRequest((url) => url.endsWith("/oauth2/token"));
    component = mountPage();
    await held.started();
    await unmount(component);
    component = mountPage();
    await wait("No secrets yet");
    const count = transport.requests.length;
    await held.release();
    expect(transport.requests).toHaveLength(count);
});

it.each(["load", "refresh"])(
    "denied %s hides records without ending IdP authentication and recovers explicitly",
    async (operation) => {
        transport.seed();
        if (operation === "load") {
            fetchApi.mockImplementation((url, init) =>
                isSnapshot(String(url), init)
                    ? Response.json({ code: "forbidden" }, { status: 403 })
                    : transport.fetch(url, init),
            );
        }
        component = mountPage();
        if (operation === "refresh") {
            await wait("First secret");
            click("First secret");
            transport.readStatus = 503;
            click("Refresh");
        }
        await wait(
            operation === "load"
                ? "Storage access denied"
                : "Database unavailable",
        );
        expect(auth.sdk?.session.status).toBe("authenticated");
        expect(target.textContent).not.toMatch(/First secret|No secrets yet/);
        expect(target.querySelector("#secret")).toBeNull();
        fetchApi.mockImplementation((url, init) => transport.fetch(url, init));
        transport.readStatus = 200;
        click("Refresh");
        await wait("First secret");
    },
);

it("bounds a silent sent write, reconciles it as uncertain, and never replays it", async () => {
    transport.seed();
    component = mountPage();
    await wait("First secret");
    click("First secret");
    let release!: (value: Response) => void;
    const response = new Promise<Response>((resolve) => {
        release = resolve;
    });
    let committed = false;
    fetchApi.mockImplementation(async (url, init) => {
        const result = await transport.fetch(url, init);
        if (String(init.body).includes("UPDATE secrets")) {
            committed = true;
            return response;
        }
        return result;
    });
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
        input("name", "Timed-out write");
        click("Save");
        await vi.waitFor(() => expect(committed).toBe(true));
        await vi.advanceTimersByTimeAsync(30_001);
    } finally {
        vi.useRealTimers();
    }
    await wait("not acknowledged; authoritative records reloaded");
    expect(list()).toContain("Timed-out write");
    expect(target.querySelector("#secret")).toBeNull();
    expect(mutations()).toHaveLength(1);
    release(Response.json({ results: [{ columns: [], rows: [] }] }));
    await tick();
    expect(target.textContent).not.toContain("Saved.");
});

it("retries interrupted schema initialization without replacing records or resetting the committed version", async () => {
    let lost = true;
    fetchApi.mockImplementation(async (url, init) => {
        const result = await transport.fetch(url, init);
        if (lost && String(init.body).includes("CREATE TABLE")) {
            lost = false;
            throw new Error("lost response");
        }
        return result;
    });
    component = mountPage();
    await wait("Storage request failed");
    transport.records.set(id, structuredClone(secret));
    click("Retry preparation");
    await wait("First secret");
    expect(
        transport.requests.filter(({ request }) =>
            request?.statements.some(({ sql }) => sql.startsWith("CREATE")),
        ),
    ).toHaveLength(1);
});

it.each([
    "missing-version",
    "duplicate-version",
    "wrong-version-id",
    "wrong-columns",
    "duplicate-record",
    "mismatched-record",
    "invalid-record",
])("fails closed on %s without exposing response content", async (failure) => {
    transport.seed();
    fetchApi.mockImplementation(async (url, init) => {
        const result = await transport.fetch(url, init);
        const body = String(init.body);
        if (
            failure.includes("version") &&
            body.includes("SELECT id, version")
        ) {
            const response = await result.json();
            const rows = response.results[0].rows;
            if (failure === "missing-version") rows.splice(0);
            else if (failure === "duplicate-version") rows.push(rows[0]);
            else rows[0][0].value = id;
            return Response.json(response);
        }
        if (isSnapshot(String(url), init)) {
            const response = await result.json();
            const row = response.results[0].rows[0];
            if (failure === "wrong-columns")
                response.results[0].columns = ["content", "id"];
            else if (failure === "duplicate-record")
                response.results[0].rows.push(row);
            else if (failure === "mismatched-record")
                row[0].value = "01900000-0000-7000-8000-000000000098";
            else if (failure === "invalid-record")
                row[1].value = JSON.stringify({ ...secret, secret: "" });
            return Response.json(response);
        }
        return result;
    });
    component = mountPage();
    await wait(
        failure.includes("version")
            ? "Incompatible or malformed"
            : "Malformed Storage",
    );
    expect(target.textContent).not.toMatch(
        /First secret|No secrets yet|older|private/,
    );
    expect(mutations()).toHaveLength(0);
});

it.each(["provisioning", "schema", "load", "refresh", "write"])(
    "identity changes during %s cannot deliver old secrets or notices to a replacement User",
    async (operation) => {
        transport.seed();
        if (operation === "refresh" || operation === "write") {
            component = mountPage();
            await wait("First secret");
            click("First secret");
        }
        const held = holdRequest((url, init) =>
            operation === "provisioning"
                ? url.endsWith("/ensure")
                : operation === "schema"
                  ? String(init.body).includes("SELECT id, version")
                  : operation === "write"
                    ? String(init.body).includes("UPDATE secrets")
                    : isSnapshot(url, init),
        );
        if (operation === "refresh") click("Refresh");
        else if (operation === "write") {
            input("name", "Old User draft");
            click("Save");
        } else component = mountPage();
        await held.started();
        userinfo.mockImplementation(async () =>
            Response.json({ sub: "user-two" }),
        );
        await expect(auth.sdk?.storage().listDatabases()).rejects.toThrow(
            "ended",
        );
        expect(target.querySelector("#secret")).toBeNull();
        expect(target.textContent).not.toContain("First secret");
        expect(held.signal?.aborted).toBe(true);
        await unmount(component);
        transport = new SqlTransport();
        auth.sdk = new ApplicationSdk(sdkOptions);
        const authorization = await auth.sdk.getAuthorizationUrl();
        await auth.sdk.handleSigninCallback(
            new URL(
                `https://app.test/callback?code=replacement&state=${authorization.searchParams.get("state")}`,
            ),
        );
        component = mountPage("user-two");
        await wait("No secrets yet");
        await held.release();
        expect(target.textContent).not.toMatch(
            /First secret|Old User draft|Saved\.|Refreshed\./,
        );
        expect(transport.records.size).toBe(0);
    },
);

it("does not expose record values or provider bodies in UI failures or application logs", async () => {
    const log = vi.spyOn(console, "log");
    const warn = vi.spyOn(console, "warn");
    const error = vi.spyOn(console, "error");
    onTestFinished(() => {
        log.mockRestore();
        warn.mockRestore();
        error.mockRestore();
    });
    transport.seed();
    component = mountPage();
    await wait("First secret");
    click("First secret");
    input("name", "Not logged draft");
    transport.uncertain = true;
    click("Save");
    await wait("not acknowledged");
    transport.malformed = true;
    click("Refresh");
    await wait("Malformed Storage record");
    const output = JSON.stringify([
        log.mock.calls,
        warn.mock.calls,
        error.mock.calls,
    ]);
    expect(output).not.toMatch(
        /Not logged draft|private|older|PRIVATE BODY|storage-token|user-token/,
    );
    expect(target.textContent).not.toMatch(
        /Not logged draft|private|older|PRIVATE BODY|storage-token|user-token/,
    );
});

it.each(["not-a-database-id", 17])(
    "rejects malformed provisioned identity %s without opening a collection",
    async (id) => {
        fetchApi.mockImplementation((url, init) =>
            String(url).endsWith("/ensure")
                ? Response.json({ id })
                : transport.fetch(url, init),
        );
        component = mountPage();
        await wait("Malformed database resource");
        expect(target.textContent).not.toContain("No secrets yet");
        expect(transport.requests).toHaveLength(0);
    },
);
