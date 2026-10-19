import { afterEach, beforeEach, expect, it, vi } from "vitest";

const host = vi.hoisted(() => ({
    invoke: vi.fn(),
    desktop: true,
    token: "current-user",
}));
vi.mock("@tauri-apps/api/core", () => ({
    invoke: host.invoke,
    isTauri: () => host.desktop,
}));
vi.mock("./oidc.svelte", () => ({
    getOidcClient: () => ({
        getStoredTokenResponse: () => ({ access_token: host.token }),
    }),
}));
vi.mock("./afterSigninRedirect.svelte", () => ({
    afterSigninRedirect: { setURL: vi.fn() },
}));
vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
vi.mock("$app/state", () => ({ page: { url: new URL("https://app.test") } }));
vi.mock("$env/dynamic/public", () => ({ env: {} }));
vi.mock("@aicacia/svelte-headless", () => ({
    createStorage: (_key: string, item: unknown) => ({ item }),
}));
const fetchApi = vi.fn();

beforeEach(() => {
    vi.resetModules();
    host.desktop = true;
    host.token = "current-user";
    host.invoke.mockReset().mockResolvedValue("https://localhost:4100");
    fetchApi.mockReset().mockImplementation(async (url: string) => {
        if (url.endsWith("/health")) return new Response(null);
        if (url.endsWith("/oauth2/token"))
            return Response.json({
                access_token: "owner-token",
                token_type: "Bearer",
            });
        if (url.endsWith("/applications")) return Response.json([]);
        if (url.endsWith("/userinfo"))
            return Response.json({ sub: "current-user" });
        if (url.endsWith("/version"))
            return Response.json({ name: "management-server" });
        if (url.endsWith("/restriction"))
            return Response.json({ admin_allowed: true });
        throw new Error(`Unexpected URL: ${url}`);
    });
    vi.stubGlobal("fetch", fetchApi);
});
afterEach(() => vi.unstubAllGlobals());

it("joins owner transport prefixes once and exchanges the distinct Management audience", async () => {
    const idp = await import("./idpClient.svelte");
    const management = await import("./managementClient.svelte");
    const { idpAdministration } = await import("./idpAdministration");
    expect(await idp.ensureTauriIdpApiUrl()).toBe("https://localhost:4100/idp");
    await management.ensureTauriManagementApiUrl();
    expect(management.getManagementApiUrl()).toBe(
        "https://localhost:4100/management",
    );
    await idp.idpApi.userinfo();
    await idpAdministration.listApplications({ offset: 0, limit: 10 });
    await management.managementApi.getDeviceRestriction({
        deviceId: "01900000-0000-7000-8000-000000000001",
    });

  const calls = fetchApi.mock.calls.filter(([url]) => !url.endsWith("/health"));
    expect(calls.map(([url]) => url)).toEqual([
        "https://localhost:4100/idp/oauth2/token",
        "https://localhost:4100/idp/userinfo",
        "https://localhost:4100/idp/oauth2/token",
        "https://localhost:4100/idp/applications",
        "https://localhost:4100/idp/oauth2/token",
        "https://localhost:4100/management/devices/01900000-0000-7000-8000-000000000001/restriction",
    ]);
    expect(
        calls
            .filter(([url]) => url.endsWith("/oauth2/token"))
            .map(([, init]) => {
                const body = init.body as URLSearchParams;
                return [
                    body.get("resource"),
                    body.get("grant_type"),
                    body.get("subject_token"),
                ];
            }),
    ).toEqual([
        [
            "https://localhost:4100/idp",
            "urn:ietf:params:oauth:grant-type:token-exchange",
            "current-user",
        ],
        [
            "https://localhost:4100/idp",
            "urn:ietf:params:oauth:grant-type:token-exchange",
            "current-user",
        ],
        [
            "idp-management",
            "urn:ietf:params:oauth:grant-type:token-exchange",
            "current-user",
        ],
    ]);
    expect(
        calls
            .filter(([url]) => !url.endsWith("/oauth2/token"))
            .map(([, init]) => init.headers.Authorization),
  ).toEqual(["Bearer owner-token", "Bearer owner-token", "Bearer owner-token"]);
});

it("exchanges the current actor independently on each Management operation", async () => {
    const management = await import("./managementClient.svelte");
    await management.ensureTauriManagementApiUrl();
    await management.managementApi.getDeviceRestriction({
        deviceId: "01900000-0000-7000-8000-000000000001",
    });
    host.token = "another-current-user";
    await management.managementApi.getDeviceRestriction({
        deviceId: "01900000-0000-7000-8000-000000000001",
    });
    expect(
        fetchApi.mock.calls
            .filter(([url]) => url.endsWith("/oauth2/token"))
            .map(([, init]) => {
                const body = init.body as URLSearchParams;
                return [body.get("resource"), body.get("subject_token")];
            }),
    ).toEqual([
        ["idp-management", "current-user"],
        ["idp-management", "another-current-user"],
    ]);
});

it("rejects a transport URL as a Management audience without sending a token", async () => {
    const { ownerAccessToken } = await import("./idpClient.svelte");
    await expect(
        ownerAccessToken("https://localhost:4100/management"),
    ).rejects.toThrow("A current IdP or Management owner audience is required");
    expect(fetchApi.mock.calls.map(([url]) => url)).toEqual([
        "https://localhost:4100/idp/health",
    ]);
});

it("keeps configured web owner prefixes without adding desktop service paths", async () => {
    host.desktop = false;
    const idp = await import("./idpClient.svelte");
    const management = await import("./managementClient.svelte");
    idp.setIdpApiUrl("https://node.test/idp");
    management.setManagementApiUrl("https://node.test/management");
    expect(await idp.ensureTauriIdpApiUrl()).toBe("https://node.test/idp");
    await management.ensureTauriManagementApiUrl();
  expect(management.getManagementApiUrl()).toBe("https://node.test/management");
    expect(
    await management.validateManagementApiUrl("https://node.test/management"),
    ).toBe(true);
    expect(fetchApi.mock.calls.map(([url]) => url)).toEqual([
        "https://node.test/management/version",
    ]);
    expect(host.invoke).not.toHaveBeenCalled();
});

const deviceId = "01900000-0000-7000-8000-000000000001";
const applicationId = "01900000-0000-7000-8000-000000000002";
const filesystemId = "01900000-0000-7000-8000-000000000003";

async function localResidency(accessToken = "filesystem-user-token") {
    const { ResourcesResidencyApi, StorageOwnerConfiguration } = await import(
        "@aicacia/storage-client"
    );
  const { ensureLocalhostBaseUrl } = await import("./localhostBaseUrl.svelte");
    return new ResourcesResidencyApi(
        new StorageOwnerConfiguration({
            basePath: `${await ensureLocalhostBaseUrl()}/storage`,
            accessToken,
            fetchApi: fetch,
        }),
    );
}

it.each(["full", "passthrough"] as const)(
    "keeps %s pending until acknowledgement, then requires an independent mode read",
    async (mode) => {
        const api = await localResidency();
        const reply = Promise.withResolvers<Response>();
        fetchApi.mockImplementationOnce(() => reply.promise);
        const acknowledged = vi.fn();
        const pending = api
            .putFileSystemResidency(
                { filesystemId, resourceResidency: { mode } },
                { redirect: "error" },
            )
            .then(acknowledged);
        await vi.waitFor(() => expect(fetchApi).toHaveBeenCalledTimes(2));
        expect(acknowledged).not.toHaveBeenCalled();
        expect(fetchApi.mock.calls[1]).toEqual([
            `https://localhost:4100/storage/filesystems/${filesystemId}/residency`,
            expect.objectContaining({
                method: "PUT",
                redirect: "error",
                headers: expect.objectContaining({
                    Authorization: "Bearer filesystem-user-token",
                }),
                body: JSON.stringify({ mode }),
            }),
        ]);
        reply.resolve(new Response(null, { status: 204 }));
        await pending;
        expect(acknowledged.mock.calls).toEqual([[undefined]]);
        expect(fetchApi).toHaveBeenCalledTimes(2);

        fetchApi.mockResolvedValueOnce(Response.json({ mode }));
        await expect(
            api.getFileSystemResidency({ filesystemId }, { redirect: "error" }),
        ).resolves.toEqual({ mode });
        expect(fetchApi.mock.calls[2][1]).toMatchObject({
            method: "GET",
            redirect: "error",
        });
    },
);

it.each(["full", "passthrough"] as const)(
    "rejects cleanup-fenced %s without acknowledgement or automatic retry",
    async (mode) => {
        const api = await localResidency();
        fetchApi.mockResolvedValueOnce(
            Response.json({ error: "cleanup_pending" }, { status: 409 }),
        );
        await expect(
            api.putFileSystemResidency(
                { filesystemId, resourceResidency: { mode } },
                { redirect: "error" },
            ),
        ).rejects.toMatchObject({ response: { status: 409 } });
        expect(fetchApi).toHaveBeenCalledTimes(2);
    expect(host.invoke.mock.calls).toEqual([["get_localhost_server_base_url"]]);
    },
);

it("does not turn deselection or Full acknowledgement into transferred-resource selection readiness", async () => {
    const { SelectionKind } = await import("@aicacia/management-client");
    const management = await import("./managementClient.svelte");
    await management.ensureTauriManagementApiUrl();
    const residency = await localResidency();
    const ownerFetch = fetchApi.getMockImplementation();
    const selectionReply = Promise.withResolvers<Response>();
    let selectionAttempts = 0;
    fetchApi.mockImplementation((url: string, init: RequestInit) => {
        if (url.endsWith("/residency"))
            return Promise.resolve(new Response(null, { status: 204 }));
    if (url.endsWith(`/selection/${applicationId}/filesystem/${filesystemId}`))
            return Promise.resolve(new Response(null, { status: 204 }));
        if (url.endsWith("/selection")) {
            selectionAttempts += 1;
            return selectionAttempts === 1
                ? selectionReply.promise
                : Promise.resolve(new Response(null, { status: 204 }));
        }
        return ownerFetch?.(url, init);
    });
    await expect(
        management.managementApi.deleteDeviceResourceSelection({
            deviceId,
            applicationId,
            kind: SelectionKind.Filesystem,
            resourceId: filesystemId,
        }),
    ).resolves.toBeUndefined();
    await expect(
        residency.putFileSystemResidency(
            { filesystemId, resourceResidency: { mode: "full" } },
            { redirect: "error" },
        ),
    ).resolves.toBeUndefined();

    const selectionRequest = {
        applicationId,
        id: filesystemId,
        kind: SelectionKind.Filesystem,
        storageAccessToken: "transferred-resource-token",
    };
    const acknowledged = vi.fn();
    const pending = management.managementApi
        .putDeviceSelection({ deviceId, selectionRequest })
        .then(acknowledged);
    const rejection = expect(pending).rejects.toMatchObject({
        response: { status: 409 },
    });
    await vi.waitFor(() => expect(selectionAttempts).toBe(1));
    expect(acknowledged).not.toHaveBeenCalled();
    selectionReply.resolve(
        Response.json({ error: "cleanup_pending" }, { status: 409 }),
    );
    await rejection;
    expect(acknowledged).not.toHaveBeenCalled();
    expect(selectionAttempts).toBe(1);

    host.token = "current-transferred-owner";
    await expect(
        management.managementApi.putDeviceSelection({
            deviceId,
            selectionRequest: {
                ...selectionRequest,
                storageAccessToken: "fresh-transferred-resource-token",
            },
        }),
    ).resolves.toBeUndefined();
    const selectionCalls = fetchApi.mock.calls.filter(([url]) =>
        url.endsWith("/selection"),
    );
    expect(selectionCalls.map(([url]) => url)).toEqual([
        `https://localhost:4100/management/devices/${deviceId}/selection`,
        `https://localhost:4100/management/devices/${deviceId}/selection`,
    ]);
    expect(selectionCalls.map(([, init]) => JSON.parse(init.body))).toEqual([
        {
            applicationId,
            id: filesystemId,
            kind: "filesystem",
            storageAccessToken: "transferred-resource-token",
        },
        {
            applicationId,
            id: filesystemId,
            kind: "filesystem",
            storageAccessToken: "fresh-transferred-resource-token",
        },
    ]);
  expect(selectionCalls[1][1].headers.Authorization).toBe("Bearer owner-token");
    const exchanges = fetchApi.mock.calls.filter(([url]) =>
        url.endsWith("/oauth2/token"),
    );
  expect(exchanges.map(([, init]) => init.body.get("subject_token"))).toEqual([
    "current-user",
    "current-user",
    "current-transferred-owner",
  ]);
    expect(exchanges.map(([, init]) => init.body.get("resource"))).toEqual([
        "idp-management",
        "idp-management",
        "idp-management",
    ]);
});

it.each([true, false])(
    "renders Residency scope without claiming observed mode or readiness (desktop=%s)",
    async (desktop) => {
        host.desktop = desktop;
        const { render } = await import("svelte/server");
        const { default: ResourceResidency } = await import(
            "../components/ResourceResidency.svelte"
        );
        const { body } = render(ResourceResidency, { props: { filesystemId } });
        expect(body).toContain(
            "Full permits local content operations but does not prove all content is available.",
        );
        expect(body).toContain("requires an authorized reachable Full peer");
        expect(body).toContain("Cleanup fences remain enforced by Storage.");
        expect(body).not.toContain("Current owner-reported local mode:");
        expect(body).not.toContain("Storage acknowledged local");
        expect(body).toMatch(
            /<button[^>]*disabled[^>]*>Apply local Residency<\/button>/,
        );
        if (!desktop) {
            expect(body).toMatch(/<fieldset[^>]*disabled/);
            expect(body).toContain("Unavailable outside the native desktop");
        }
        expect(fetchApi).not.toHaveBeenCalled();
        expect(host.invoke).not.toHaveBeenCalled();
    },
);

it("renders Device policy as owner-authorized mutations, not a ready resource directory", async () => {
    const { render } = await import("svelte/server");
    const { default: DevicePolicy } = await import(
        "../components/DevicePolicy.svelte"
    );
    const { body } = render(DevicePolicy, { props: { devices: [] } });
    expect(body).toContain("signing in or joining does not grant access.");
    expect(body).toContain(
        "Mutation acknowledgements above are not presented as a current selection list.",
    );
    expect(body).toContain(
        "Full/Passthrough Residency controls apply to whole filesystems, not databases.",
    );
    expect(body).toMatch(
        /<button[^>]*disabled[^>]*>Select authorized resource<\/button>/,
    );
    expect(body).not.toContain("Management acknowledged selection");
    expect(fetchApi).not.toHaveBeenCalled();
});

it("uses the Management prefix once and audience for web source acceptance and exact offer decisions without native IPC", async () => {
    host.desktop = false;
    const idp = await import("./idpClient.svelte");
    const management = await import("./managementClient.svelte");
    idp.setIdpApiUrl("https://identity.test/idp");
    management.setManagementApiUrl("https://source.test/management");
    const offers = [
        {
            offer_id: "01900000-0000-7000-8000-000000000004",
            authenticated_endpoint: "b".repeat(64),
            request: {
                installation: "https://installation.test",
                member_id: "01900000-0000-7000-8000-000000000005",
                endpoint_id: "b".repeat(64),
                approving_peer: "a".repeat(64),
                name: "Joining Node",
                public_signer: { kty: "OKP" },
            },
        },
    ];
    fetchApi.mockImplementation(async (url: string) => {
        if (url.endsWith("/oauth2/token"))
            return Response.json({
                access_token: "owner-token",
                token_type: "Bearer",
            });
    if (url.endsWith("/acceptance")) return Response.json({ accepting: true });
        if (url.endsWith("/pending")) return Response.json(offers);
        if (url.endsWith("/approve"))
            return Response.json({ request: offers[0].request });
        return new Response(null, { status: 204 });
    });
    const devices = await import("./devices.svelte");
    await expect(devices.nodeJoinAccepting(true)).resolves.toBe(true);
    await expect(devices.pendingNodeJoins()).resolves.toEqual(offers);
    await devices.approvePendingNodeJoin(offers[0].offer_id);
    await devices.rejectPendingNodeJoin(offers[0].offer_id);
    const calls = fetchApi.mock.calls.filter(([url]) =>
        url.includes("/node-joins/"),
    );
    expect(calls.map(([url, init]) => [url, init.method, init.body])).toEqual([
        [
            "https://source.test/management/node-joins/acceptance",
            "PUT",
            '{"accepting":true}',
        ],
        ["https://source.test/management/node-joins/pending", "GET", undefined],
        [
            "https://source.test/management/node-joins/01900000-0000-7000-8000-000000000004/approve",
            "POST",
            undefined,
        ],
        [
            "https://source.test/management/node-joins/01900000-0000-7000-8000-000000000004/reject",
            "POST",
            undefined,
        ],
    ]);
    for (const [, init] of calls)
        expect(init).toMatchObject({
            headers: { Authorization: "Bearer owner-token" },
            credentials: "omit",
            redirect: "error",
            cache: "no-store",
        });
    expect(
        fetchApi.mock.calls
            .filter(([url]) => url.endsWith("/oauth2/token"))
            .map(([url, init]) => [url, init.body.get("resource")]),
    ).toEqual(
    Array(4).fill(["https://identity.test/idp/oauth2/token", "idp-management"]),
    );
    expect(host.invoke).not.toHaveBeenCalled();
});

it("web source approval refuses missing User authorization, insecure targets and malformed offer identity", async () => {
    host.desktop = false;
    const idp = await import("./idpClient.svelte");
    const devices = await import("./devices.svelte");
    const management = await import("./managementClient.svelte");
    idp.setIdpApiUrl("https://identity.test/idp");
    management.setManagementApiUrl("http://source.test/management");
    await expect(devices.nodeJoinAccepting(true)).rejects.toThrow(/HTTPS/);
    expect(fetchApi).not.toHaveBeenCalled();
    management.setManagementApiUrl("https://source.test/management");
    host.token = "";
    await expect(devices.pendingNodeJoins()).rejects.toThrow(/Sign in/);
    expect(fetchApi).not.toHaveBeenCalled();
    await expect(
        devices.approvePendingNodeJoin("../acceptance"),
    ).rejects.toThrow();
    expect(fetchApi).not.toHaveBeenCalled();
    expect(host.invoke).not.toHaveBeenCalled();
});

it.each(["https://management.test", "https://node.test/management"])(
    "uses the configured source Management base %s without inventing a prefix",
    async (base) => {
        host.desktop = false;
        const idp = await import("./idpClient.svelte");
        const management = await import("./managementClient.svelte");
        const devices = await import("./devices.svelte");
        idp.setIdpApiUrl("https://identity.test/idp");
        management.setManagementApiUrl(base);
        fetchApi.mockImplementation(async (url: string) =>
            url.endsWith("/oauth2/token")
                ? Response.json({
                      access_token: "management-token",
                      token_type: "Bearer",
                  })
                : Response.json({ accepting: false }),
        );
        await expect(devices.nodeJoinAccepting()).resolves.toBe(false);
        expect(fetchApi.mock.calls.map(([url]) => url)).toEqual([
            "https://identity.test/idp/oauth2/token",
            `${base}/node-joins/acceptance`,
        ]);
        expect(fetchApi.mock.calls[0][1].body.get("resource")).toBe(
            "idp-management",
        );
        expect(fetchApi.mock.calls[1][1].headers.Authorization).toBe(
            "Bearer management-token",
        );
    },
);

it("does not transfer source approval to a changed Management target during token exchange", async () => {
    host.desktop = false;
    const idp = await import("./idpClient.svelte");
    const management = await import("./managementClient.svelte");
    const devices = await import("./devices.svelte");
    idp.setIdpApiUrl("https://identity.test/idp");
    management.setManagementApiUrl("https://source.test/management");
    fetchApi.mockImplementation(async () => {
        management.setManagementApiUrl("https://other.test/management");
        return Response.json({
            access_token: "management-token",
            token_type: "Bearer",
        });
    });
    await expect(devices.nodeJoinAccepting(true)).rejects.toThrow(
        /target changed/,
    );
    expect(fetchApi.mock.calls.map(([url]) => url)).toEqual([
        "https://identity.test/idp/oauth2/token",
    ]);
});

it("uses Management HTTPS approval and audience on bundled desktop without removed native commands", async () => {
  const ownerFetch = fetchApi.getMockImplementation();
  fetchApi.mockImplementation((url, init) =>
    url.endsWith("/acceptance")
      ? Response.json({ accepting: true })
      : ownerFetch?.(url, init),
  );
    const devices = await import("./devices.svelte");
    await expect(devices.nodeJoinAccepting(true)).resolves.toBe(true);
    expect(
        fetchApi.mock.calls
            .find(([url]) => url.endsWith("/oauth2/token"))?.[1]
            .body.get("resource"),
    ).toBe("idp-management");
  expect(
    fetchApi.mock.calls.find(([url]) => url.endsWith("/acceptance"))?.[0],
  ).toBe("https://localhost:4100/management/node-joins/acceptance");
  expect(
    host.invoke.mock.calls.every(
      ([command]) => command === "get_localhost_server_base_url",
    ),
  ).toBe(true);
});

it("uses Management Device client routes and audience for list, rename and revoke", async () => {
    host.desktop = false;
    const idp = await import("./idpClient.svelte");
    const management = await import("./managementClient.svelte");
    const devices = await import("./devices.svelte");
    idp.setIdpApiUrl("https://identity.test/idp");
    management.setManagementApiUrl("https://devices.test/management");
    fetchApi.mockImplementation(async (url: string, init: RequestInit) => {
        if (url.endsWith("/oauth2/token"))
            return Response.json({
                access_token: "management-token",
                token_type: "Bearer",
            });
        if (init.method === "GET") return Response.json([]);
        if (init.method === "PATCH")
            return Response.json({ id: deviceId, name: "Renamed" });
        return new Response(null, { status: 204 });
    });
    await expect(devices.listDevices()).resolves.toEqual([]);
    await devices.renameDevice(deviceId, "Renamed");
    await devices.revokeDevice(deviceId);
    const calls = fetchApi.mock.calls.filter(
        ([url]) => !url.endsWith("/oauth2/token"),
    );
    expect(calls.map(([url, init]) => [url, init.method])).toEqual([
        ["https://devices.test/management/devices", "GET"],
        [`https://devices.test/management/devices/${deviceId}`, "PATCH"],
        [`https://devices.test/management/devices/${deviceId}`, "DELETE"],
    ]);
    expect(calls[1][1].body).toBe(JSON.stringify({ name: "Renamed" }));
    for (const [, init] of calls)
        expect(init.headers.Authorization).toBe("Bearer management-token");
    expect(
        fetchApi.mock.calls
            .filter(([url]) => url.endsWith("/oauth2/token"))
            .map(([, init]) => init.body.get("resource")),
    ).toEqual(["idp-management", "idp-management", "idp-management"]);
});

it("handles independent source acceptance 204 with readback, exact offers and asynchronous approval 202", async () => {
  host.desktop = false;
  const idp = await import("./idpClient.svelte");
  const management = await import("./managementClient.svelte");
  idp.setIdpApiUrl("https://identity.test/idp");
  management.setManagementApiUrl("https://source.test/management");
  const operation = {
    purpose: "join",
    operation_nonce: "1".repeat(32),
    administrator: "",
    node: "Joining",
    idp: "https://target.test/idp",
    management: "https://target.test/management",
    storage: "https://target.test/storage",
    issuer: "https://installation.test/idp",
    peer: "https://source.test/management#" + "a".repeat(64),
  };
  const request = {
    operation,
    device_id: "01900000-0000-7000-8000-000000000003",
    member_id: "01900000-0000-7000-8000-000000000004",
    endpoint_id: "b".repeat(64),
    public_signer: { kid: "01900000-0000-7000-8000-000000000005" },
    source: {
      endpoint: "a".repeat(64),
      management: "https://source.test/management",
      issuer: operation.issuer,
    },
  };
  const offerId = "01900000-0000-7000-8000-000000000006";
  fetchApi.mockImplementation(async (url: string, init: RequestInit) => {
    if (url.endsWith("/oauth2/token"))
      return Response.json({
        access_token: "owner-token",
        token_type: "Bearer",
      });
    if (url.endsWith("/acceptance"))
      return init.method === "PUT"
        ? new Response(null, { status: 204 })
        : Response.json({ accepting: true });
    if (url.endsWith("/pending"))
      return Response.json([
        {
          offer_id: offerId,
          authenticated_endpoint: request.endpoint_id,
          request,
        },
      ]);
    return new Response(null, {
      status: url.endsWith("/approve") ? 202 : 204,
    });
  });
  const devices = await import("./devices.svelte");
  await expect(devices.nodeJoinAccepting(true)).resolves.toBe(true);
  const offers = await devices.pendingNodeJoins();
  expect(offers[0].request).toMatchObject({
    operation,
    device_id: request.device_id,
    member_id: request.member_id,
    name: "Joining",
    installation: operation.issuer,
  });
  await expect(devices.approvePendingNodeJoin(offerId)).resolves.toBe(
    "accepted",
  );
  await devices.rejectPendingNodeJoin(offerId);
  expect(
    fetchApi.mock.calls
      .filter(([url]) => url.endsWith("/acceptance"))
      .map(([, init]) => init.method),
  ).toEqual(["PUT", "GET"]);
  expect(host.invoke).not.toHaveBeenCalled();
});

it("keeps desktop remote owner targets and source approval on HTTPS without invoking unrelated native setup", async () => {
  const idp = await import("./idpClient.svelte");
  const management = await import("./managementClient.svelte");
  idp.setIdpApiUrl("https://remote.test/idp");
  management.setManagementApiUrl("https://remote.test/management");
  expect(await idp.ensureTauriIdpApiUrl()).toBe("https://remote.test/idp");
  await management.ensureTauriManagementApiUrl();
  expect(management.getManagementApiUrl()).toBe(
    "https://remote.test/management",
  );
  fetchApi.mockImplementation(async (url: string) =>
    url.endsWith("/oauth2/token")
      ? Response.json({
          access_token: "owner-token",
          token_type: "Bearer",
        })
      : Response.json({ accepting: true }),
  );
  const devices = await import("./devices.svelte");
  await expect(devices.nodeJoinAccepting()).resolves.toBe(true);
  expect(fetchApi.mock.calls.map(([url]) => url)).toEqual([
    "https://remote.test/idp/oauth2/token",
    "https://remote.test/management/node-joins/acceptance",
  ]);
  expect(host.invoke).not.toHaveBeenCalled();
});
