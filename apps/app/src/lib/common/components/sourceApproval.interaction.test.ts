import { flushSync, mount, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import SourceApproval from "./SourceApproval.svelte";

vi.mock("../state/idpClient.svelte", () => ({
    ownerAccessToken: async () => "management-token",
    ensureTauriIdpApiUrl: async () => {},
    getIdpApiUrl: () => "https://source.test/idp",
}));
vi.mock("../state/deviceClient.svelte", () => ({
    deviceApi: {},
    ensureTauriDeviceApiUrl: async () => {},
    getDeviceApiUrl: () => "https://source.test/device",
}));
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
afterEach(async () => {
    if (component) await unmount(component);
    target?.remove();
    vi.unstubAllGlobals();
});
function button(label: string) {
    const found = [...target.querySelectorAll("button")].find((node) =>
        node.textContent?.includes(label),
    );
    if (!found) throw new Error("Missing button: " + label);
    return found;
}
it("guides acceptance and shows the trusted endpoint in the same add-device task", async () => {
    let accepting = false;
    const fetchApi = vi.fn(async (url: string, init: RequestInit = {}) => {
        if (url.endsWith("/setup/status"))
            return Response.json({
                stage: "installation",
                state: "awaiting_choice",
                ready: false,
                owners: ["idp", "device", "storage"],
            });
        if (url.endsWith("/pending")) return Response.json([]);
        if (url.endsWith("/acceptance")) {
            if (init.method === "PUT")
                accepting = JSON.parse(init.body as string).accepting;
            return Response.json({ accepting });
        }
        throw new Error("Unexpected URL: " + url);
    });
    vi.stubGlobal("fetch", fetchApi);
    target = document.createElement("div");
    document.body.append(target);
    component = mount(SourceApproval, {
        target,
        props: { endpoint: "trusted-source-endpoint" },
    });
    flushSync();
    await vi.waitFor(() =>
        expect(button("Allow requests").disabled).toBe(false),
    );
    expect(target.textContent).toContain("Add a device");
    expect(target.textContent).toContain("Join an existing Installation");
    expect(target.textContent).toContain("trusted-source-endpoint");
    button("Allow requests").click();
    flushSync();
    await vi.waitFor(() =>
        expect(button("Stop accepting").disabled).toBe(false),
    );
    expect(target.textContent).toContain("Waiting for a request");
    button("Stop accepting").click();
    flushSync();
    await vi.waitFor(() =>
        expect(button("Allow requests").disabled).toBe(false),
    );
    expect(target.textContent).toContain("Requests are closed");
    const write = fetchApi.mock.calls.find(
        ([, init]) => init?.method === "PUT",
    )!;
    expect(write[0]).toBe("https://source.test/device/node-joins/acceptance");
    expect((write[1].headers as Record<string, string>).Authorization).toBe(
        "Bearer management-token",
    );
});

const offerId = "01900000-0000-7000-8000-000000000001";
const otherOfferId = "01900000-0000-7000-8000-000000000002";
const member = "01900000-0000-7000-8000-000000000003";
const device = "01900000-0000-7000-8000-000000000004";
function offer(id: string, independent: boolean) {
    const common = {
        member_id: member,
        endpoint_id: "authenticated-target",
        public_signer: { kid: "independent-signer" },
    };
    return {
        offer_id: id,
        authenticated_endpoint: "authenticated-target",
        request: independent
            ? {
                  ...common,
                  device_id: device,
                  source: {
                      endpoint:
                          "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                      device: "https://source.test/device",
                      issuer: "https://installation.test/idp",
                  },
                  operation: {
                      purpose: "join",
                      operation_nonce: "11111111111111111111111111111111",
                      administrator: "",
                      issuer: "https://installation.test/idp",
                      node: "Laptop",
                      peer: "https://source.test/device#aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                      idp: "https://target.test/idp",
                      device: "https://target.test/device",
                      storage: "https://target.test/storage",
                  },
              }
            : {
                  ...common,
                  installation: "https://installation.test/idp",
                  approving_peer:
                      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                  name: "Laptop",
              },
    };
}
async function openOffers(
    independent: boolean,
    decision: (url: string) => Response,
) {
    const fetchApi = vi.fn(async (url: string, _init: RequestInit = {}) => {
        if (url.endsWith("/setup/status"))
            return Response.json({
                stage: "installation",
                state: "awaiting_choice",
                ready: false,
                owners: independent ? ["idp"] : ["idp", "device", "storage"],
            });
        if (url.endsWith("/pending"))
            return Response.json([
                offer(offerId, independent),
                offer(otherOfferId, independent),
            ]);
        if (url.endsWith("/acceptance"))
            return Response.json({ accepting: true });
        return decision(url);
    });
    vi.stubGlobal("fetch", fetchApi);
    vi.stubGlobal(
        "confirm",
        vi.fn(() => true),
    );
    target = document.createElement("div");
    document.body.append(target);
    component = mount(SourceApproval, {
        target,
        props: {
            endpoint:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        },
    });
    flushSync();
    await vi.waitFor(() =>
        expect(target.querySelectorAll("article")).toHaveLength(2),
    );
    return fetchApi;
}
it.each([false, true])(
    "approves and rejects only the selected exact request (independent=%s)",
    async (independent) => {
        const fetchApi = await openOffers(
            independent,
            (url) =>
                new Response(null, {
                    status: url.endsWith("/approve") ? 202 : 204,
                }),
        );
        const articles = target.querySelectorAll("article");
        expect(articles[0].textContent).toContain(offerId);
        expect(articles[0].textContent).toContain(
            "https://installation.test/idp",
        );
        expect(articles[0].textContent).toContain("authenticated-target");
        expect(articles[0].textContent).toContain("independent-signer");
        if (independent) {
            expect(articles[0].textContent).toContain(device);
            expect(articles[0].textContent).toContain(
                "https://target.test/storage",
            );
        }
        articles[1].querySelector("button")!.click();
        flushSync();
        await vi.waitFor(() =>
            expect(
                target.querySelector('[role="status"]')?.textContent,
            ).toContain("Approval accepted"),
        );
        expect(target.textContent).toContain("not proof it is ready");
        expect(target.querySelector('a[href="/signin"]')).toBeNull();
        await vi.waitFor(() =>
            expect(button("Approve this device").disabled).toBe(false),
        );
        articles[0].querySelectorAll("button")[1].click();
        flushSync();
        await vi.waitFor(() =>
            expect(
                target.querySelector('[role="status"]')?.textContent,
            ).toContain("rejection acknowledged"),
        );
        const prefix = independent ? "device/independent" : "device";
        expect(
            fetchApi.mock.calls
                .filter(([url]) => /\/(approve|reject)$/.test(url))
                .map(([url]) => url),
        ).toEqual([
            `https://source.test/${prefix}/node-joins/${otherOfferId}/approve`,
            `https://source.test/${prefix}/node-joins/${offerId}/reject`,
        ]);
    },
);
it("keeps partial enrollment failure visible across refresh and retries exactly the same offer", async () => {
    let attempts = 0;
    const fetchApi = await openOffers(
        true,
        () => new Response(null, { status: ++attempts === 1 ? 503 : 200 }),
    );
    button("Approve this device").click();
    flushSync();
    await vi.waitFor(() =>
        expect(target.querySelector('[role="alert"]')?.textContent).toContain(
            "partly applied",
        ),
    );
    button("Refresh requests").click();
    flushSync();
    await vi.waitFor(() =>
        expect(button("Refresh requests").disabled).toBe(false),
    );
    expect(target.querySelector('[role="alert"]')?.textContent).toContain(
        "partly applied",
    );
    button("Retry this approval").click();
    flushSync();
    await vi.waitFor(() =>
        expect(target.querySelector('[role="status"]')?.textContent).toContain(
            "Source approval recorded",
        ),
    );
    expect(
        fetchApi.mock.calls
            .filter(([url]) => url.endsWith("/approve"))
            .map(([url]) => url),
    ).toEqual([
        `https://source.test/device/independent/node-joins/${offerId}/approve`,
        `https://source.test/device/independent/node-joins/${offerId}/approve`,
    ]);
});
it.each([401, 403])(
    "shows actionable unauthorized owner responses (%s) without offering a decision",
    async (status) => {
        vi.stubGlobal(
            "fetch",
            vi.fn(async (url: string) =>
                url.endsWith("/setup/status")
                    ? Response.json({
                          stage: "installation",
                          state: "awaiting_choice",
                          ready: false,
                          owners: ["idp"],
                      })
                    : new Response(null, { status }),
            ),
        );
        target = document.createElement("div");
        document.body.append(target);
        component = mount(SourceApproval, { target });
        flushSync();
        await vi.waitFor(() =>
            expect(
                target.querySelector('[role="alert"]')?.textContent,
            ).toContain("signing in with permission"),
        );
        expect(target.querySelectorAll("article")).toHaveLength(0);
    },
);
