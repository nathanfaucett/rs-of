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
    getIdpApiUrl: () => "https://target.test/idp",
    usesRemoteApis: () => !native.desktop,
    ensureTauriIdpApiUrl: async () => "https://target.test/idp",
}));
vi.mock("../state/deviceClient.svelte", () => ({
    getDeviceApiUrl: () => "https://target.test/device",
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

it.each([
    ["create", false],
    ["join", false],
    ["create", true],
    ["join", true],
] as const)(
    "drives owner %s (bundled=%s) through transient exact requests and correlated readiness",
    async (purpose, bundled) => {
        native.desktop = bundled;
        native.invoke.mockImplementation(async (command, args) =>
            command === "get_localhost_server_base_url"
                ? "https://target.test"
                : (args.owner === "idp"
                      ? "a"
                      : args.owner === "device"
                        ? "b"
                        : "c"
                  ).repeat(64),
        );
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
        component = mount(IndependentSetup, {
            target,
            props: { hosts, bundled },
        });
        flushSync();
        enter("Installation setup choice", purpose);
        if (!bundled)
            for (let index = 0; index < owners.length; index++)
                enter(
                    owners[index] + " host setup code",
                    String.fromCharCode(97 + index).repeat(64),
                );
        if (purpose === "join") enter("Node name", "Interactive Node");
        else expect(target.textContent).not.toContain("Node name");
        if (purpose === "create") {
            enter("Administrator username", "Alice");
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
            expect(url).toBe(bases[owners[index]] + "/setup/owner/" + purpose);
            expect((init.headers as Record<string, string>).Authorization).toBe(
                "Setup " + String.fromCharCode(97 + index).repeat(64),
            );
            const body = JSON.parse(init.body as string);
            expect(body.node).toBe(
                purpose === "create" ? "My Node" : "Interactive Node",
            );
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
        if (bundled)
            expect(
                native.invoke.mock.calls
                    .filter(([command]) => command === "get_native_setup_code")
                    .map(([, args]) => args.owner),
            ).toEqual(owners);
        else expect(native.invoke).not.toHaveBeenCalled();
    },
);
