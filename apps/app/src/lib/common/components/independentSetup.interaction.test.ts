import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import IndependentSetup from "./IndependentSetup.svelte";
import type { SetupHost } from "../state/setupApi";

const native = vi.hoisted(() => ({ desktop: false, invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({
    isTauri: () => native.desktop,
    invoke: native.invoke,
}));
vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("../state/oidc.svelte", () => ({
    clearOidcSession: vi.fn(),
    getOidcClient: () => ({ getStoredTokenResponse: () => undefined }),
}));
vi.mock("../state/idpClient.svelte", () => ({
    getIdpApiUrl: () => "https://localhost:4100/idp",
    usesRemoteApis: () => !native.desktop,
    ensureTauriIdpApiUrl: async () => "https://localhost:4100/idp",
}));
vi.mock("../state/deviceClient.svelte", () => ({
    getDeviceApiUrl: () => "https://localhost:4100/device",
    ensureTauriDeviceApiUrl: async () => {},
}));
vi.mock("$env/dynamic/public", () => ({ env: {} }));
beforeEach(() => {
    native.desktop = false;
    native.invoke.mockReset();
});
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
const bases = {
    idp: "https://target.test/idp",
    device: "https://target.test/device",
    storage: "https://target.test/storage",
};
const owners = ["idp", "device", "storage"] as const;
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
afterEach(async () => {
    if (component) await unmount(component);
    target?.remove();
    vi.unstubAllGlobals();
});
function input(label: string) {
    const element = [...target.querySelectorAll("label")]
        .find((node) => node.textContent?.includes(label))
        ?.querySelector<HTMLInputElement | HTMLSelectElement>("input,select");
    if (!element) throw new Error("Missing input " + label);
    return element;
}
function enter(label: string, value: string) {
    const element = input(label);
    element.value = value;
    element.dispatchEvent(
        new Event(element.tagName === "SELECT" ? "change" : "input", {
            bubbles: true,
        }),
    );
    flushSync();
}
function button(label: string) {
    return [...target.querySelectorAll("button")].find((node) =>
        node.textContent?.includes(label),
    )!;
}

it.each(["create", "join"] as const)(
    "drives independent %s from UI through transient inputs, exact requests and correlated readiness",
    async (purpose) => {
        target = document.createElement("div");
        document.body.append(target);
        let operation: Record<string, unknown> | undefined;
        let ready = false;
        const fetchApi = vi.fn(async (url: string, init: RequestInit = {}) => {
            const owner = owners.find((owner) =>
                url.startsWith(bases[owner] + "/"),
            )!;
            if (url.endsWith("/status"))
                return Response.json({
                    stage: "installation",
                    state: ready ? "ready" : "awaiting_choice",
                    ready,
                    owners: [owner],
                });
            if (url.endsWith("/public"))
                return Response.json({
                    owner,
                    operation,
                    facts:
                        owner === "idp"
                            ? {
                                  login_client:
                                      "01900000-0000-7000-8000-000000000001",
                              }
                            : {},
                });
            const { password: _password, ...intent } = JSON.parse(
                init.body as string,
            );
            operation = { ...intent, issuer: "https://installation.test/idp" };
            if (purpose === "create") operation.issuer = bases.idp;
            return new Response(null, { status: 202 });
        });
        vi.stubGlobal("fetch", fetchApi);
        const hosts: SetupHost[] = owners.map((owner) => ({
            base: bases[owner],
            status: {
                stage: "installation",
                state: "awaiting_choice",
                ready: false,
                owners: [owner],
            },
        }));
        component = mount(IndependentSetup, { target, props: { hosts } });
        flushSync();
        enter("Installation setup choice", purpose);
        for (let index = 0; index < owners.length; index++)
            enter(
                owners[index] + " host setup code",
                String.fromCharCode(97 + index).repeat(64),
            );
        if (purpose === "join") enter("Node name", "Interactive Node");
        else expect(target.textContent).not.toContain("Node name");
        if (purpose === "create") {
            enter("Administrator name", "Alice");
            enter("IdP administrator password", "protected-password");
        } else
            enter(
                "Trusted source",
                "https://source.test/device#" + "d".repeat(64),
            );
        target
            .querySelector("form")!
            .dispatchEvent(
                new Event("submit", { bubbles: true, cancelable: true }),
            );
        flushSync();
        expect(
            [
                ...target.querySelectorAll<HTMLInputElement>(
                    'input[type="password"]',
                ),
            ].every((node) => node.value === ""),
        ).toBe(true);
        await vi.waitFor(() =>
            expect(
                fetchApi.mock.calls.filter(([url]) =>
                    url.endsWith("/" + purpose),
                ),
            ).toHaveLength(3),
        );
        const requests = fetchApi.mock.calls.filter(([url]) =>
            url.endsWith("/" + purpose),
        );
        for (let index = 0; index < requests.length; index++) {
            const [url, init] = requests[index];
            expect(url).toBe(
                bases[owners[index]] + "/independent/setup/" + purpose,
            );
            expect((init.headers as Record<string, string>).Authorization).toBe(
                "Setup " + String.fromCharCode(97 + index).repeat(64),
            );
            const body = JSON.parse(init.body as string);
            expect(body.node).toBe(purpose === "create" ? "My Node" : "Interactive Node");
            expect(body.administrator).toBe(
                purpose === "create" ? "Alice" : "",
            );
            expect("password" in body).toBe(
                purpose === "create" && index === 0,
            );
        }
        expect(target.querySelector('a[href="/signin"]')).toBeNull();
        ready = true;
        await vi.waitFor(() =>
            expect(button("Observe all owners").disabled).toBe(false),
        );
        button("Observe all owners").click();
        flushSync();
        await vi.waitFor(() =>
            expect(target.querySelector('a[href="/signin"]')).not.toBeNull(),
        );
        expect(target.textContent).toContain("same exact operation and issuer");
    },
);

it.each(["create", "join"] as const)(
    "drives bundled desktop %s through protected code retrieval and the same HTTPS form",
    async (purpose) => {
        native.desktop = true;
        const { default: Installation } = await import(
            "../../../routes/setup/installation/+page.svelte"
        );
        let ready = false;
        native.invoke.mockImplementation(async (command) => {
            if (command === "pending_reset_revocations") return [];
            if (command === "get_onboarding_host")
                return {
                    base: "https://localhost:4100",

                    status: {
                        stage: "installation",
                        state: ready
                            ? "ready"
                            : purpose === "join"
                              ? "joining"
                              : "creating",
                        ready,
                        owners: ["idp", "device", "storage"],
                        ...(ready
                            ? {
                                  login_client: {
                                      client_id:
                                          "01900000-0000-7000-8000-000000000001",
                                      scopes: ["openid"],
                                  },
                              }
                            : {}),
                    },
                };
            if (command === "get_localhost_server_base_url")
                return "https://localhost:4100";
            if (command === "get_native_setup_code") return "a".repeat(64);
            throw new Error("Unexpected native command: " + command);
        });
        target = document.createElement("div");
        document.body.append(target);
        const fetchApi = vi.fn(async (url: string, _init?: RequestInit) => {
            const status = {
                stage: "installation",
                state: ready
                    ? "ready"
                    : purpose === "create"
                      ? "creating"
                      : "joining",
                ready,
                owners: ["idp", "device", "storage"],
                ...(ready
                    ? {
                          login_client: {
                              client_id: "01900000-0000-7000-8000-000000000001",
                              scopes: ["openid"],
                          },
                      }
                    : {}),
            };
            return Response.json(status, {
                status: url.endsWith("/status") ? 200 : 202,
            });
        });
        vi.stubGlobal("fetch", fetchApi);
        const hosts: SetupHost[] = [
            {
                base: "https://localhost:4100/idp",
                status: {
                    stage: "installation",
                    state: "awaiting_choice",
                    ready: false,
                    owners: ["idp", "device", "storage"],
                },
            },
        ];
        component = mount(Installation, {
            target,
            props: {
                data: {
                    observation: {
                        state: "needs_setup",
                        action: "choose",
                        hosts,
                        target: {
                            kind: "bundled",
                            idp: hosts[0].base,
                            device: "https://localhost:4100/device",
                            storage: "https://localhost:4100/storage",
                        },
                    },
                },
            },
        });
        flushSync();
        enter("Installation setup choice", purpose);
        if (purpose === "create") {
            enter("Administrator name", "Alice");
            enter("Password", "protected-password");
        } else enter("Trusted peer endpoint ID", "d".repeat(64));
        if (purpose === "join") {
            enter("Node name", "Bundled Node");
            button("Get bundled host code").click();
            await vi.waitFor(() => expect(input("Host setup code").value).toBe("a".repeat(64)));
        } else {
            expect(target.textContent).not.toContain("Host setup code");
            expect(target.textContent).not.toContain("Node name");
            expect(target.querySelector('a[href="/config"]')?.textContent).toContain("Advanced");
        }
        target
            .querySelector("form")!
            .dispatchEvent(
                new Event("submit", { bubbles: true, cancelable: true }),
            );
        flushSync();
        await vi.waitFor(() => expect(fetchApi).toHaveBeenCalledTimes(1));
        expect(fetchApi.mock.calls[0][0]).toBe(
            "https://localhost:4100/idp/setup/" + purpose,
        );
        const init = fetchApi.mock.calls[0][1] as RequestInit;
        expect((init.headers as Record<string, string>).Authorization).toBe(
            "Setup " + "a".repeat(64),
        );
        const body = JSON.parse(init.body as string);
        expect(body.node).toBe(purpose === "create" ? "My Node" : "Bundled Node");
        expect("password" in body).toBe(purpose === "create");
        if (purpose === "join") expect(input("Host setup code").value).toBe("");
        expect(target.querySelector('a[href="/signin"]')).toBeNull();
        ready = true;
        await vi.waitFor(() =>
            expect(button("Refresh status").disabled).toBe(false),
        );
        button("Refresh status").click();
        flushSync();
        await vi.waitFor(() =>
            expect(
                target.querySelector('a[href="/signin"]'),
            ).not.toBeNull(),
        );
        expect(
            native.invoke.mock.calls.every(([command]) =>
                [
                    "get_onboarding_host",
                    "pending_reset_revocations",
                    "get_localhost_server_base_url",
                    "get_native_setup_code",
                ].includes(command),
            ),
        ).toBe(true);
    },
);

it("retries a failed bundled runtime without resetting retained identity", async () => {
    native.desktop = true;
    const { default: Installation } = await import(
        "../../../routes/setup/installation/+page.svelte"
    );
    native.invoke.mockImplementation(async (command) => {
        if (command === "pending_reset_revocations") return [];
        if (command === "retry_local_runtime") return;
        if (command === "get_onboarding_host")
            return {
                base: "https://localhost:4100",
                status: {
                    stage: "installation",
                    state: "awaiting_choice",
                    ready: false,
                    owners: ["idp", "device", "storage"],
                },
            };
        throw new Error("Unexpected command: " + command);
    });
    target = document.createElement("div");
    document.body.append(target);
    component = mount(Installation, {
        target,
        props: {
            data: {
                observation: {
                    state: "blocked",
                    action: "retry",
                    message: "Unable to verify current readiness.",
                    hosts: [],
                    target: {
                        kind: "bundled",
                        idp: "https://localhost:4100/idp",
                        device: null,
                        storage: null,
                    },
                },
            },
        },
    });
    flushSync();
    button("Retry local services").click();
    flushSync();
    await vi.waitFor(() =>
        expect(target.textContent).toContain(
            "Create a new Installation or join an existing one.",
        ),
    );
    expect(native.invoke.mock.calls.map(([command]) => command)).toContain(
        "retry_local_runtime",
    );
    expect(native.invoke.mock.calls.map(([command]) => command)).not.toContain(
        "reset_device",
    );
});


it("recovers an accepted Create after closure and retries the retained identity with a newly entered password", async () => {
    native.desktop = true;
    const { default: Installation } = await import("../../../routes/setup/installation/+page.svelte");
    let retained: Record<string, string> | undefined;
    const requests: Record<string, string>[] = [];
    const status = () => ({
        stage: "installation", state: retained ? "creating" : "awaiting_choice",
        ready: false, owners: ["idp", "device", "storage"],
    });
    native.invoke.mockImplementation(async (command) => {
        if (command === "pending_reset_revocations") return [];
        if (command === "get_native_setup_code") return "a".repeat(64);
        if (command === "get_localhost_server_base_url") return "https://localhost:4100";
        if (command === "get_onboarding_host") return { base: "https://localhost:4100", status: status() };
        throw new Error("Unexpected native command: " + command);
    });
    const fetchApi = vi.fn(async (url: string, init: RequestInit = {}) => {
        if (url.endsWith("/intent")) return Response.json(retained);
        if (url.endsWith("/status")) return Response.json(status());
        if (!url.endsWith("/create")) throw new Error("Unexpected owner mutation: " + url);
        const body = JSON.parse(init.body as string);
        requests.push(body);
        const { password: _password, ...intent } = body;
        retained = intent;
        if (requests.length === 1) throw new Error("response lost after durable acceptance");
        return Response.json(status(), { status: 202 });
    });
    vi.stubGlobal("fetch", fetchApi);
    target = document.createElement("div");
    document.body.append(target);
    const open = () => {
        const hosts: SetupHost[] = [{ base: "https://localhost:4100/idp", status: status() as SetupHost["status"] }];
        component = mount(Installation, { target, props: { data: { observation: {
            ...(retained ? { state: "working" as const, action: "observe" as const } : { state: "needs_setup" as const, action: "choose" as const }), hosts,
            target: { kind: "bundled", idp: hosts[0].base, device: null, storage: null },
        } } } });
        flushSync();
    };
    open();
    enter("Administrator name", "Alice");
    enter("Password", "original-password");
    target.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.waitFor(() => expect(requests).toHaveLength(1));
    await vi.waitFor(() => expect(button("Resume setup").disabled).toBe(false));
    expect(target.querySelector('a[href="/signin"]')).toBeNull();
    await unmount(component);
    target.replaceChildren();
    open();
    expect(target.textContent).toContain("Creating your Node");
    expect(target.querySelector('input[autocomplete="new-password"]')).toBeNull();
    button("Resume setup").click();
    await vi.waitFor(() => expect(input("Administrator name").value).toBe("Alice"));
    expect(input("Password").value).toBe("");
    expect(button("Retry exact Create").disabled).toBe(true);
    enter("Password", "original-password");
    target.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.waitFor(() => expect(requests).toHaveLength(2));
    expect(requests[1]).toEqual(requests[0]);
    expect(retained).not.toHaveProperty("password");
    expect(input("Password").value).toBe("");
    expect(target.querySelector('a[href="/signin"]')).toBeNull();
});
