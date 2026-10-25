import { SvelteMap } from "svelte/reactivity";
import Page from "../../../routes/(auth)/+page.svelte";
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { StorageRequest } from "@aicacia/storage-client";
import Browser from "./Browser.svelte";

vi.mock("$app/navigation", () => ({ goto: vi.fn(), invalidateAll: vi.fn() }));
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
const auth = vi.hoisted(() => ({ token: "user-token", clear: vi.fn() }));
vi.mock("$env/dynamic/public", () => ({
    env: { PUBLIC_STORAGE_BASE_URL: "https://storage.test/storage" },
}));
vi.mock("$lib/common/state/idpClient.svelte", () => ({
    getIdpApiUrl: () => "https://idp.test/idp",
}));
vi.mock("$lib/common/state/oidc.svelte", () => ({
    getOidcClient: () => ({
        getAccessToken: async () => auth.token,
        clearStoredTokenResponse: auth.clear,
    }),
}));

const id = "01900000-0000-7000-8000-000000000001";
const secret = {
    id,
    name: "First secret",
    secret: "private",
    favorite: false,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
};
const entry = (name: string) => ({ name, hash: "h", size: 1, local: true });
class Endpoint extends EventTarget {
    static OPEN = 1;
    static sockets: Endpoint[] = [];
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
        const response =
            request.type === "list"
                ? {
                      type: "listed",
                      entries:
                          request.path === ""
                              ? fs === "empty"
                                  ? []
                                  : [entry("secrets")]
                              : [entry(`secrets/${id}.json`)],
                  }
                : {
                      type: "read",
                      content: [
                          ...new TextEncoder().encode(JSON.stringify(secret)),
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
    Endpoint.sockets = [];
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
            .every((button) => button.disabled),
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

it("clears records and editor when the authenticated User changes and on sign out", async () => {
    const identity = new SvelteMap([["subject", "user-one"]]);
    component = mount(Page, {
        target,
        props: {
            params: {},
            get data() {
                return { userInfo: { sub: identity.get("subject") } };
            },
        },
    });
    await wait("Duplicate name");
    select("first");
    await wait("First secret");
    Array.from(target.querySelectorAll("button"))
        .find((button) => button.textContent?.includes("First secret"))
        ?.click();
    flushSync();
    expect(target.querySelector("#secret")).not.toBeNull();
    auth.token = "second-user-token";
    flushSync(() => identity.set("subject", "user-two"));
    await wait("Duplicate name");
    expect(target.textContent).not.toContain("First secret");
    expect(target.querySelector("#secret")).toBeNull();
    expect(Endpoint.sockets[0]?.closed).toBe(true);
    select("first");
    await wait("First secret");
    Array.from(target.querySelectorAll("button"))
        .find((button) => button.textContent?.includes("Sign out"))
        ?.click();
    flushSync();
    expect(target.textContent).not.toContain("First secret");
    expect(Endpoint.sockets[1]?.closed).toBe(true);
    expect(auth.clear).toHaveBeenCalledOnce();
});
