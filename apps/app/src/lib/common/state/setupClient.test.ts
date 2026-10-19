import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SetupCreateIntent, SetupHost } from "./setupApi";

const retainedCreate: SetupCreateIntent = {
  operation_nonce: "1".repeat(32),
  administrator: "retained admin",
  node: "retained node",
};
const recoveryHost: SetupHost = {
  base: "https://node.test/idp",
  status: {
    stage: "installation",
    state: "unfinished",
    ready: false,
    owners: ["idp", "management", "storage"],
  },
};

const host = vi.hoisted(() => ({
  authority: null as string | null,
  management: "https://policy.test/api" as string | null,
  native: true,
  invoke: vi.fn(),
  submitCreate: vi.fn(),
  cancelCreate: vi.fn(),
  recoverIntent: vi.fn(),
}));
vi.mock("./setupApi", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./setupApi")>()),
  submitSetupCreate: host.submitCreate,
  cancelSetupCreate: host.cancelCreate,
  fetchSetupIntent: host.recoverIntent,
}));
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => host.native,
  invoke: host.invoke,
}));
vi.mock("./idpClient.svelte", () => ({
  getIdpApiUrl: () => host.authority,
  usesApiSetup: () => !host.native,
}));
vi.mock("./managementClient.svelte", () => ({
  getManagementApiUrl: () => host.management,
}));
vi.mock("$env/dynamic/public", () => ({
  env: {
    PUBLIC_URL: "https://app.test",
    PUBLIC_STORAGE_BASE_URL: "https://data.test/api",
  },
}));

const entries = new Map<string, string>();
const storage: Storage = {
  get length() {
    return entries.size;
  },
  key: (index) => [...entries.keys()][index] ?? null,
  getItem: (key) => entries.get(key) ?? null,
  setItem: (key, value) => {
    entries.set(key, value);
  },
  removeItem: (key) => {
    entries.delete(key);
  },
  clear: () => entries.clear(),
};
const fetchApi = vi.fn();

beforeEach(() => {
  vi.resetModules();
  host.authority = null;
  host.management = "https://policy.test/api";
  host.native = true;
  host.invoke.mockReset();
  host.submitCreate.mockReset();
  host.cancelCreate.mockReset();
  host.recoverIntent.mockReset();
  fetchApi.mockReset();
  entries.clear();
  vi.stubGlobal("window", {
    localStorage: storage,
    location: new URL("https://app.test"),
  });
  vi.stubGlobal("fetch", fetchApi);
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

it("retains exact Join after uncertainty, observes before retry, recovers issuer before cancellation and never cancels on drop", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate({
    ...recoveryHost,
    status: { ...recoveryHost.status, state: "awaiting_choice" },
  });
  fetchApi.mockRejectedValueOnce(new Error("lost response"));
  await session.join("a".repeat(64), "Joined Node", "b".repeat(64));
  const intent = session.intent;
  expect(intent).toMatchObject({ node: "Joined Node", peer: "b".repeat(64) });
  expect(session.observationRequired).toBe(true);
  await session.join("a".repeat(64), "Joined Node", "b".repeat(64));
  expect(fetchApi).toHaveBeenCalledTimes(1);
  await session.cancel("a".repeat(64));
  expect(fetchApi).toHaveBeenCalledTimes(1);
  fetchApi.mockResolvedValueOnce(
    Response.json({
      ...recoveryHost.status,
      stage: "installation",
      state: "joining",
    }),
  );
  await session.observe();
  await session.join("a".repeat(64), "Joined Node", "c".repeat(64));
  expect(fetchApi).toHaveBeenCalledTimes(2);
  fetchApi.mockResolvedValueOnce(
    Response.json(
      { ...recoveryHost.status, stage: "installation", state: "joining" },
      { status: 202 },
    ),
  );
  await session.join("a".repeat(64), "Joined Node", "b".repeat(64));
  expect(JSON.parse(fetchApi.mock.calls[2][1].body)).toEqual(intent);
  host.recoverIntent.mockResolvedValueOnce({
    ...intent,
    issuer: "https://installation.test",
  });
  fetchApi.mockResolvedValueOnce(
    Response.json({
      ...recoveryHost.status,
      stage: "installation",
      state: "joining",
    }),
  );
  await session.recover("a".repeat(64));
  expect(session.intent).toEqual({
    ...intent,
    issuer: "https://installation.test",
  });
  fetchApi.mockResolvedValueOnce(new Response(null, { status: 204 }));
  await session.cancel("a".repeat(64));
  expect(JSON.parse(fetchApi.mock.calls[4][1].body)).toEqual({
    ...intent,
    issuer: "https://installation.test",
  });
  expect(session.status?.ready).toBe(false);
  session.destroy();
  expect(fetchApi).toHaveBeenCalledTimes(5);
  expect(entries.size).toBe(0);
  expect(host.invoke).not.toHaveBeenCalled();
});

it("serializes unified Join and dropping the submitting client never sends cancellation", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate({
    ...recoveryHost,
    status: { ...recoveryHost.status, state: "awaiting_choice" },
  });
  const reply = Promise.withResolvers<Response>();
  fetchApi.mockReturnValueOnce(reply.promise);
  const pending = session.join("a".repeat(64), "Joined Node", "b".repeat(64));
  await session.join("a".repeat(64), "Other Node", "c".repeat(64));
  await session.recover("a".repeat(64));
  await session.cancel("a".repeat(64));
  expect(fetchApi).toHaveBeenCalledTimes(1);
  expect(host.recoverIntent).not.toHaveBeenCalled();
  session.destroy();
  reply.resolve(
    Response.json(
      { ...recoveryHost.status, state: "joining" },
      { status: 202 },
    ),
  );
  await pending;
  expect(fetchApi).toHaveBeenCalledTimes(1);
  expect(session.intent).toBeNull();
  expect(session.message).toBe("");
  expect(session.status?.ready).toBe(false);
  expect(host.cancelCreate).not.toHaveBeenCalled();
  expect(host.invoke).not.toHaveBeenCalled();
});

it("recovers a reloaded Join without generating identity and rejects changed issuer or purpose", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate(recoveryHost);
  const retained = {
    operation_nonce: "2".repeat(32),
    node: "Joined Node",
    peer: "b".repeat(64),
    issuer: "https://installation.test",
  };
  const random = vi.spyOn(crypto, "getRandomValues");
  host.recoverIntent.mockResolvedValueOnce(retained);
  fetchApi.mockResolvedValueOnce(Response.json(recoveryHost.status));
  await session.recover("a".repeat(64));
  expect(session.intent).toEqual(retained);
  expect(random).not.toHaveBeenCalled();
  await session.submit("a".repeat(64), "admin", retained.node, "password");
  expect(host.submitCreate).not.toHaveBeenCalled();
  host.recoverIntent.mockResolvedValueOnce({
    ...retained,
    issuer: "https://other.test",
  });
  fetchApi.mockResolvedValueOnce(Response.json(recoveryHost.status));
  await session.recover("a".repeat(64));
  expect(session.intent).toEqual(retained);
  expect(session.observationRequired).toBe(true);
  await session.cancel("a".repeat(64));
  expect(fetchApi).toHaveBeenCalledTimes(2);
  expect(host.cancelCreate).not.toHaveBeenCalled();
  expect(host.invoke).not.toHaveBeenCalled();
});

it("resets an incomplete native Node with no OIDC authority or live services", async () => {
  storage.setItem("oidc.token:https://old-node.test:response", "old session");
  storage.setItem(
    "oidc.pkce:https://old-node.test:state:pending",
    "old verifier",
  );
  storage.setItem("idp-api-url", "null");
  const { getOidcClient } = await import("./oidc.svelte");
  expect(() => getOidcClient()).toThrow("Invalid authority URL");
  const { resetDevice } = await import("./setupClient.svelte");
  host.invoke.mockImplementation(async (command) => {
    expect(command).toBe("reset_device");
    expect([...entries.keys()]).toEqual(["idp-api-url"]);
  });

  await expect(resetDevice()).resolves.toBeUndefined();
  expect(host.invoke.mock.calls).toEqual([["reset_device"]]);
  expect(fetchApi).not.toHaveBeenCalled();
});

it("clears the existing native memory session and PKCE state and invalidates desktop client identity", async () => {
  host.authority = "https://node.test/idp";
  host.invoke.mockResolvedValueOnce("old-client");
  const { ensureOidcClientId, getOidcClient } = await import("./oidc.svelte");
  expect(await ensureOidcClientId()).toBe("old-client");
  expect(await ensureOidcClientId()).toBe("old-client");
  const client = getOidcClient();
  client.setToken({ access_token: "session", refresh_token: "refresh" });
  fetchApi.mockResolvedValue(
    Response.json({
      authorization_endpoint: "https://node.test/idp/oauth2/authorize",
    }),
  );
  await client.getAuthorizationUrl({ state: "pending" });
  expect(client.getStoredTokenResponse()?.access_token).toBe("session");
  storage.setItem("oidc.token:https://old-node.test:response", "old session");
  storage.setItem("oidc.pkce:https://node.test/idp:last", "old verifier");
  storage.setItem("unrelated", "keep");
  host.authority = null;
  fetchApi.mockClear();
  host.invoke.mockImplementation(async (command) => {
    if (command === "get_desktop_client_id") return "new-client";
    expect(command).toBe("reset_device");
    expect(client.getStoredTokenResponse()).toBeNull();
    expect([...entries]).toEqual([["unrelated", "keep"]]);
  });

  const { resetDevice } = await import("./setupClient.svelte");
  await resetDevice();
  await expect(
    client.handleSigninCallback(
      new URL("https://app.test/callback?code=old-code&state=pending"),
    ),
  ).rejects.toMatchObject({ code: "INVALID_STATE" });
  expect(await ensureOidcClientId()).toBe("new-client");
  expect(host.invoke.mock.calls).toEqual([
    ["get_desktop_client_id"],
    ["reset_device"],
    ["get_desktop_client_id"],
  ]);
  expect(fetchApi).not.toHaveBeenCalled();
});

it("propagates a real native reset failure and permits an explicit retry", async () => {
  const failure = new Error(
    "Local secret deletion failed; reset remains pending",
  );
  host.invoke.mockRejectedValueOnce(failure).mockResolvedValueOnce(undefined);
  const { resetDevice } = await import("./setupClient.svelte");
  await expect(resetDevice()).rejects.toBe(failure);
  await expect(resetDevice()).resolves.toBeUndefined();
  expect(host.invoke.mock.calls).toEqual([["reset_device"], ["reset_device"]]);
  expect(fetchApi).not.toHaveBeenCalled();
});

it.each(["installation", "device", "ready", "unavailable"])(
  "obtains durable native setup stage %s independently of HTTP listener availability",
  async (stage) => {
    host.invoke.mockResolvedValue(stage);
    const { getSetupStage } = await import("./setupClient.svelte");
    expect(await getSetupStage()).toBe(stage);
    expect(host.invoke.mock.calls).toEqual([["get_setup_stage"]]);
    expect(fetchApi).not.toHaveBeenCalled();
  },
);

it("does not manufacture readiness when native setup evidence is unavailable", async () => {
  const failure = new Error("setup manifest is invalid");
  host.invoke.mockRejectedValue(failure);
  const { getSetupStage } = await import("./setupClient.svelte");
  await expect(getSetupStage()).rejects.toBe(failure);
  expect(fetchApi).not.toHaveBeenCalled();
});

it("reads web setup stage through the configured API without native IPC", async () => {
  host.native = false;
  host.authority = "https://node.test/idp";
  fetchApi.mockResolvedValue(
    Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    }),
  );
  const { getSetupStage } = await import("./setupClient.svelte");
  expect(await getSetupStage()).toBe("installation");
  expect(host.invoke).not.toHaveBeenCalled();
  expect(fetchApi).toHaveBeenCalledWith("https://node.test/idp/setup/status", {
    credentials: "omit",
    redirect: "error",
    cache: "no-store",
  });
});

it("web sign-in discovers the retained public login Client rather than hard-coded idp-web", async () => {
  host.native = false;
  host.authority = "https://node.test/idp";
  fetchApi.mockResolvedValue(
    Response.json({
      stage: "installation",
      state: "ready",
      ready: true,
      owners: ["idp", "management", "storage"],
      login_client: {
        client_id: "01234567-89ab-4def-8123-456789abcdef",
        scopes: ["openid", "profile", "email"],
      },
    }),
  );
  const { ensureOidcClientId } = await import("./oidc.svelte");
  expect(await ensureOidcClientId()).toBe(
    "01234567-89ab-4def-8123-456789abcdef",
  );
  expect(fetchApi).toHaveBeenCalledWith("https://node.test/idp/setup/status", {
    credentials: "omit",
    redirect: "error",
    cache: "no-store",
  });
  expect(host.invoke).not.toHaveBeenCalled();
  expect(entries.size).toBe(0);
});

it("web sign-in rejects missing current readiness without stale Client reuse", async () => {
  host.native = false;
  host.authority = "https://node.test/idp";
  fetchApi.mockResolvedValue(
    Response.json({
      stage: "installation",
      state: "creating",
      ready: false,
      owners: ["idp", "management", "storage"],
    }),
  );
  const { ensureOidcClientId } = await import("./oidc.svelte");
  await expect(ensureOidcClientId()).rejects.toThrow("public login Client");
});

it("verifies the web host code without persisting it or invoking native setup", async () => {
  host.native = false;
  host.authority = "https://node.test/idp";
  fetchApi
    .mockResolvedValueOnce(
      Response.json({
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners: ["idp", "management", "storage"],
      }),
    )
    .mockResolvedValueOnce(new Response(null, { status: 204 }));
  const { verifyWebSetupCode } = await import("./setupClient.svelte");
  await verifyWebSetupCode("https://node.test/idp", "a".repeat(64));
  expect(host.invoke).not.toHaveBeenCalled();
  expect(entries.size).toBe(0);
  expect(fetchApi).toHaveBeenCalledWith(
    "https://node.test/idp/setup/admission",
    {
      method: "POST",
      headers: { Authorization: `Setup ${"a".repeat(64)}` },
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
    },
  );
});

it("does not swallow native persistent session clearing failures", async () => {
  const failure = new Error("Session storage unavailable");
  vi.stubGlobal("window", {
    get localStorage() {
      throw failure;
    },
  });
  const { resetDevice } = await import("./setupClient.svelte");
  await expect(resetDevice()).rejects.toBe(failure);
  expect(host.invoke).not.toHaveBeenCalled();
});

it.each([false, true])(
  "preserves authority-scoped web token clearing with an existing client: %s",
  async (existing) => {
    host.native = false;
    host.authority = "https://node.test/idp/";
    storage.setItem("oidc.token:https://node.test/idp:response", "session");
    storage.setItem("oidc.token:https://other.test:response", "other session");
    storage.setItem(
      "oidc.pkce:https://node.test/idp:state:pending",
      "verifier",
    );
    const { clearOidcSession, getOidcClient } = await import("./oidc.svelte");
    const client = existing ? getOidcClient() : undefined;
    clearOidcSession();
    expect(
      storage.getItem("oidc.token:https://node.test/idp:response"),
    ).toBeNull();
    expect(client?.getStoredTokenResponse() ?? null).toBeNull();
    expect([...entries]).toEqual([
      ["oidc.token:https://other.test:response", "other session"],
      ["oidc.pkce:https://node.test/idp:state:pending", "verifier"],
    ]);
    expect(host.invoke).not.toHaveBeenCalled();
    expect(fetchApi).not.toHaveBeenCalled();
  },
);

it("discovers three configured web owners and verifies only the selected host's code without OAuth", async () => {
  host.native = false;
  host.authority = "https://node.test/idp";
  const bases = [host.authority, host.management, "https://data.test/api"];
  fetchApi.mockImplementation(async (url, options) => {
    if (options.method === "POST") return new Response(null, { status: 204 });
    const index = bases.findIndex((base) => url === `${base}/setup/status`);
    return Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: [["idp"], ["management"], ["storage"]][index],
    });
  });
  const { getWebSetupHosts, verifyWebSetupCode } =
    await import("./setupClient.svelte");
  expect((await getWebSetupHosts()).map((host) => host.base)).toEqual(bases);
  await verifyWebSetupCode("https://policy.test/api", "b".repeat(64));
  await verifyWebSetupCode("https://data.test/api", "c".repeat(64));
  const posts = fetchApi.mock.calls.filter(
    ([, options]) => options.method === "POST",
  );
  expect(posts).toEqual([
    [
      "https://policy.test/api/setup/admission",
      {
        method: "POST",
        headers: { Authorization: `Setup ${"b".repeat(64)}` },
        credentials: "omit",
        redirect: "error",
        cache: "no-store",
      },
    ],
    [
      "https://data.test/api/setup/admission",
      {
        method: "POST",
        headers: { Authorization: `Setup ${"c".repeat(64)}` },
        credentials: "omit",
        redirect: "error",
        cache: "no-store",
      },
    ],
  ]);
  expect(host.invoke).not.toHaveBeenCalled();
  expect(entries.size).toBe(0);
});

it("denies a missing independent base, stale target or unhealthy host without admission", async () => {
  host.native = false;
  host.authority = "https://node.test/idp";
  host.management = null;
  fetchApi.mockResolvedValue(
    Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp"],
    }),
  );
  const { getWebSetupHosts, verifyWebSetupCode } =
    await import("./setupClient.svelte");
  await expect(getWebSetupHosts()).rejects.toThrow(
    "configured Management and Storage",
  );
  fetchApi.mockResolvedValue(
    Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    }),
  );
  await expect(
    verifyWebSetupCode("https://other.test/idp", "a".repeat(64)),
  ).rejects.toThrow("no longer configured");
  fetchApi.mockResolvedValue(
    Response.json(
      {
        stage: "installation",
        state: "unavailable",
        ready: false,
        owners: ["idp", "management", "storage"],
      },
      { status: 503 },
    ),
  );
  await expect(
    verifyWebSetupCode(host.authority, "a".repeat(64)),
  ).rejects.toThrow("unavailable");
  expect(
    fetchApi.mock.calls.every(([, options]) => options.method !== "POST"),
  ).toBe(true);
  expect(host.invoke).not.toHaveBeenCalled();
  expect(entries.size).toBe(0);
});

it("retains one secure exact Create intent after uncertainty and requires observation before retry", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate({
    base: "https://node.test/idp",
    status: {
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    },
  });
  const random = vi.spyOn(crypto, "getRandomValues");
  host.submitCreate.mockRejectedValueOnce(new Error("secret response"));
  await session.submit("a".repeat(64), "admin", "my node", "password");
  const intent = session.intent;
  expect(intent).toMatchObject({ administrator: "admin", node: "my node" });
  expect(intent?.operation_nonce).toMatch(/^[a-f0-9]{32}$/);
  expect(random).toHaveBeenCalledWith(expect.any(Uint8Array));
  expect(random.mock.calls[0][0]?.byteLength).toBe(16);
  expect(session.message).not.toContain("secret response");
  expect(session.observationRequired).toBe(true);
  await session.submit("a".repeat(64), "admin", "my node", "password");
  expect(host.submitCreate).toHaveBeenCalledTimes(1);
  fetchApi.mockResolvedValue(
    Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    }),
  );
  await session.observe();
  host.submitCreate.mockResolvedValue({
    stage: "installation",
    state: "creating",
    ready: false,
    owners: ["idp", "management", "storage"],
  });
  await session.submit("b".repeat(64), "admin", "my node", "password");
  expect(host.submitCreate.mock.calls[1]).toEqual([
    "https://node.test/idp",
    "b".repeat(64),
    { ...intent, password: "password" },
  ]);
  expect(session.intent).toEqual(intent);
  expect(random).toHaveBeenCalledTimes(1);
  expect(entries.size).toBe(0);
  expect(host.invoke).not.toHaveBeenCalled();
  random.mockRestore();
});

it("cancels only an exact session intent with a newly supplied code and never claims cleanup", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate({
    base: "https://node.test/idp",
    status: {
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    },
  });
  host.submitCreate.mockResolvedValue({
    stage: "installation",
    state: "creating",
    ready: false,
    owners: ["idp", "management", "storage"],
  });
  await session.submit("a".repeat(64), "admin", "node", "password");
  const intent = session.intent;
  await session.cancel("invalid");
  expect(host.cancelCreate).not.toHaveBeenCalled();
  host.cancelCreate.mockResolvedValue(undefined);
  await session.cancel("b".repeat(64));
  expect(host.cancelCreate.mock.calls).toEqual([
    ["https://node.test/idp", "b".repeat(64), intent],
  ]);
  expect(session.intent).toEqual(intent);
  expect(session.status?.ready).toBe(false);
  expect(session.observationRequired).toBe(true);
  expect(session.message).toContain("not cleanup completion");
  expect(entries.size).toBe(0);
  session.destroy();
  expect(session.intent).toBeNull();
  expect(host.cancelCreate).toHaveBeenCalledTimes(1);
});

it("serializes submits, refuses conflicting session choices and destruction never cancels host-owned work", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate({
    base: "https://node.test/idp",
    status: {
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    },
  });
  let resolve!: (status: unknown) => void;
  host.submitCreate.mockImplementation(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const pending = session.submit("a".repeat(64), "admin", "node", "password");
  await session.submit("a".repeat(64), "other admin", "node", "password");
  expect(host.submitCreate).toHaveBeenCalledTimes(1);
  session.destroy();
  resolve({
    stage: "installation",
    state: "creating",
    ready: false,
    owners: ["idp", "management", "storage"],
  });
  await pending;
  expect(session.intent).toBeNull();
  expect(session.message).toBe("");
  await session.submit("a".repeat(64), "admin", "node", "password");
  await session.cancel("a".repeat(64));
  expect(host.submitCreate).toHaveBeenCalledTimes(1);
  expect(host.cancelCreate).not.toHaveBeenCalled();
  expect(entries.size).toBe(0);
});

it("clears the session intent only on verified completion and never retains credentials", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate({
    base: "https://node.test/idp",
    status: {
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    },
  });
  host.submitCreate.mockResolvedValueOnce({
    stage: "installation",
    state: "unfinished",
    ready: false,
    owners: ["idp", "management", "storage"],
  });
  await session.submit("a".repeat(64), "admin", "node", "private-password");
  const intent = session.intent;
  await session.submit("a".repeat(64), "changed", "node", "private-password");
  expect(host.submitCreate).toHaveBeenCalledTimes(1);
  expect(session.intent).toEqual(intent);
  expect(JSON.stringify(session)).not.toContain("private-password");
  expect(JSON.stringify(session)).not.toContain("a".repeat(64));
  host.submitCreate.mockResolvedValueOnce({
    stage: "ready",
    state: "ready",
    ready: true,
    owners: ["idp", "management", "storage"],
    login_client: { client_id: "public-login", scopes: ["openid"] },
  });
  await session.submit("b".repeat(64), "admin", "node", "private-password");
  expect(session.status?.ready).toBe(true);
  expect(session.intent).toBeNull();
  expect(session.message).toContain("readiness verified");
  expect(entries.size).toBe(0);
  expect(host.invoke).not.toHaveBeenCalled();
});

it.each(["creating", "unfinished", "unavailable"] as const)(
  "cannot invent a retry or cancellation intent for reloaded %s work",
  async (state) => {
    host.native = false;
    const { WebSetupCreate } = await import("./setupClient.svelte");
    const session = new WebSetupCreate({
      base: "https://node.test/idp",
      status: {
        stage: state === "unavailable" ? "unavailable" : "installation",
        state,
        ready: false,
        owners: ["idp", "management", "storage"],
      },
    });
    const random = vi.spyOn(crypto, "getRandomValues");
    await session.submit("a".repeat(64), "admin", "node", "password");
    await session.cancel("a".repeat(64));
    expect(session.intent).toBeNull();
    expect(random).not.toHaveBeenCalled();
    expect(host.submitCreate).not.toHaveBeenCalled();
    expect(host.cancelCreate).not.toHaveBeenCalled();
    random.mockRestore();
  },
);

it("does not merge independent owner scopes or assume no effect when observation also fails", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const independent = new WebSetupCreate({
    base: "https://node.test/idp",
    status: {
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp"],
    },
  });
  await independent.submit("a".repeat(64), "admin", "node", "password");
  expect(host.submitCreate).not.toHaveBeenCalled();
  const session = new WebSetupCreate({
    base: "https://node.test/idp",
    status: {
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    },
  });
  host.submitCreate.mockRejectedValue(new Error("private-password"));
  await session.submit("a".repeat(64), "admin", "node", "password");
  const intent = session.intent;
  fetchApi.mockRejectedValue(new Error("private-code"));
  await session.observe();
  await session.submit("a".repeat(64), "admin", "node", "password");
  expect(host.submitCreate).toHaveBeenCalledTimes(1);
  expect(session.intent).toEqual(intent);
  expect(session.observationRequired).toBe(true);
  expect(session.message).not.toContain("private-");
  expect(session.status?.ready).toBe(false);
});

it("fails closed on changed owner evidence without clearing the uncertain exact intent", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate({
    base: "https://node.test/idp",
    status: {
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    },
  });
  host.submitCreate.mockRejectedValue(new Error("network failure"));
  await session.submit("a".repeat(64), "admin", "node", "password");
  const intent = session.intent;
  fetchApi.mockResolvedValue(
    Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp"],
    }),
  );
  await session.observe();
  expect(session.intent).toEqual(intent);
  expect(session.observationRequired).toBe(true);
  expect(session.status?.owners).toEqual(["idp", "management", "storage"]);
  await session.submit("a".repeat(64), "admin", "node", "password");
  expect(host.submitCreate).toHaveBeenCalledTimes(1);
});

it("has no insecure nonce fallback when secure randomness is unavailable", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate({
    base: "https://node.test/idp",
    status: {
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    },
  });
  vi.spyOn(crypto, "getRandomValues").mockImplementation(() => {
    throw new Error("provider detail");
  });
  await session.submit("a".repeat(64), "admin", "node", "password");
  expect(session.intent).toBeNull();
  expect(host.submitCreate).not.toHaveBeenCalled();
  expect(session.message).not.toContain("provider detail");
});

it("retains the exact intent when explicit cancellation has an uncertain response", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate({
    base: "https://node.test/idp",
    status: {
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    },
  });
  host.submitCreate.mockResolvedValue({
    stage: "installation",
    state: "creating",
    ready: false,
    owners: ["idp", "management", "storage"],
  });
  await session.submit("a".repeat(64), "admin", "node", "password");
  const intent = session.intent;
  host.cancelCreate.mockRejectedValue(new Error("private-code"));
  await session.cancel("b".repeat(64));
  expect(session.intent).toEqual(intent);
  expect(session.status?.ready).toBe(false);
  expect(session.observationRequired).toBe(true);
  expect(session.message).toContain("do not assume setup stopped");
  expect(session.message).not.toContain("private-code");
});

it.each([
  [" admin", "node", "password"],
  ["admin", "n".repeat(129), "password"],
  ["admin", "node", "password\n"],
  ["admin", "node", "p".repeat(1025)],
  ["admin", "é".repeat(65), "password"],
])(
  "does not retain a nonce for invalid protected input",
  async (administrator, node, password) => {
    host.native = false;
    const { WebSetupCreate } = await import("./setupClient.svelte");
    const session = new WebSetupCreate({
      base: "https://node.test/idp",
      status: {
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners: ["idp", "management", "storage"],
      },
    });
    await session.submit("a".repeat(64), administrator, node, password);
    expect(session.intent).toBeNull();
    expect(host.submitCreate).not.toHaveBeenCalled();
  },
);

it.each(["creating", "unfinished"] as const)(
  "recovers exact %s Create intent through authorization then current status without generating a nonce",
  async (state) => {
    host.native = false;
    const { WebSetupCreate } = await import("./setupClient.svelte");
    const session = new WebSetupCreate({
      base: "https://node.test/idp",
      status: {
        stage: "installation",
        state,
        ready: false,
        owners: ["idp", "management", "storage"],
      },
    });
    const retained = {
      operation_nonce: "1".repeat(32),
      administrator: "retained admin",
      node: "retained node",
    };
    const random = vi.spyOn(crypto, "getRandomValues");
    host.recoverIntent.mockImplementation(async () => {
      expect(fetchApi).not.toHaveBeenCalled();
      return retained;
    });
    fetchApi.mockResolvedValue(
      Response.json({
        stage: "installation",
        state,
        ready: false,
        owners: ["storage", "idp", "management"],
      }),
    );
    await session.recover("a".repeat(64));
    expect(host.recoverIntent.mock.calls).toEqual([
      ["https://node.test/idp", "a".repeat(64)],
    ]);
    expect(fetchApi).toHaveBeenCalledWith(
      "https://node.test/idp/setup/status",
      { credentials: "omit", redirect: "error", cache: "no-store" },
    );
    expect(session.intent).toEqual(retained);
    expect(session.status?.state).toBe(state);
    expect(session.observationRequired).toBe(false);
    host.submitCreate.mockResolvedValue({
      stage: "installation",
      state: "creating",
      ready: false,
      owners: ["idp", "management", "storage"],
    });
    await session.submit(
      "b".repeat(64),
      retained.administrator,
      retained.node,
      "original-password",
    );
    expect(host.submitCreate.mock.calls).toEqual([
      [
        "https://node.test/idp",
        "b".repeat(64),
        { ...retained, password: "original-password" },
      ],
    ]);
    host.cancelCreate.mockResolvedValue(undefined);
    await session.cancel("c".repeat(64));
    expect(host.cancelCreate.mock.calls).toEqual([
      ["https://node.test/idp", "c".repeat(64), retained],
    ]);
    expect(random).not.toHaveBeenCalled();
    expect(entries.size).toBe(0);
    expect(JSON.stringify(session)).not.toContain("original-password");
    expect(JSON.stringify(session)).not.toContain("a".repeat(64));
    expect(host.invoke).not.toHaveBeenCalled();
  },
);

it.each(["operation_nonce", "administrator", "node"] as const)(
  "preserves the exact session intent on recovered %s conflict",
  async (field) => {
    host.native = false;
    const { WebSetupCreate } = await import("./setupClient.svelte");
    const session = new WebSetupCreate(recoveryHost);
    host.recoverIntent
      .mockResolvedValueOnce(retainedCreate)
      .mockResolvedValueOnce({
        ...retainedCreate,
        [field]: field === "operation_nonce" ? "2".repeat(32) : "different",
      });
    fetchApi.mockImplementation(async () => Response.json(recoveryHost.status));
    await session.recover("a".repeat(64));
    const original = session.intent;
    await session.recover("b".repeat(64));
    expect(session.intent).toBe(original);
    expect(session.intent).toEqual(retainedCreate);
    expect(session.observationRequired).toBe(true);
    expect(session.message).toContain("Existing intent is unchanged");
    expect(host.submitCreate).not.toHaveBeenCalled();
    expect(host.cancelCreate).not.toHaveBeenCalled();
  },
);

it("retains the original uncertain session intent when authorized recovery reports unbound", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate({
    ...recoveryHost,
    status: { ...recoveryHost.status, state: "awaiting_choice" },
  });
  host.submitCreate.mockRejectedValueOnce(new Error("uncertain request"));
  await session.submit("a".repeat(64), "admin", "node", "password");
  const original = session.intent;
  const random = vi.spyOn(crypto, "getRandomValues");
  host.recoverIntent.mockResolvedValue(null);
  fetchApi.mockImplementation(async () =>
    Response.json({ ...recoveryHost.status, state: "awaiting_choice" }),
  );
  await session.recover("b".repeat(64));
  expect(session.intent).toBe(original);
  expect(session.observationRequired).toBe(false);
  expect(session.message).toContain("may not yet be admitted");
  host.submitCreate.mockResolvedValue(recoveryHost.status);
  await session.submit("c".repeat(64), "admin", "node", "password");
  expect(host.submitCreate.mock.calls[1]).toEqual([
    recoveryHost.base,
    "c".repeat(64),
    { ...original, password: "password" },
  ]);
  expect(random).not.toHaveBeenCalled();
  expect(entries.size).toBe(0);
});

it("does not invent a session intent when recovery is unbound", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate(recoveryHost);
  const random = vi.spyOn(crypto, "getRandomValues");
  host.recoverIntent.mockResolvedValue(null);
  fetchApi.mockImplementation(async () =>
    Response.json({ ...recoveryHost.status, state: "awaiting_choice" }),
  );
  await session.recover("a".repeat(64));
  expect(session.intent).toBeNull();
  expect(session.status?.state).toBe("awaiting_choice");
  expect(session.message).toContain("No nonce was generated");
  expect(random).not.toHaveBeenCalled();
  expect(host.submitCreate).not.toHaveBeenCalled();
  expect(host.cancelCreate).not.toHaveBeenCalled();
});

it.each([
  "invalid or retired (401)",
  "malformed protected tuple (503)",
  "malformed intent",
  "network uncertainty",
])(
  "preserves exact intent and hides secrets/errors after recovery lookup failure: %s",
  async (failure) => {
    host.native = false;
    const { WebSetupCreate } = await import("./setupClient.svelte");
    const session = new WebSetupCreate(recoveryHost);
    host.recoverIntent
      .mockResolvedValueOnce(retainedCreate)
      .mockRejectedValueOnce(
        new Error(`${failure}: private-password ${"b".repeat(64)}`),
      );
    fetchApi.mockImplementation(async () => Response.json(recoveryHost.status));
    await session.recover("a".repeat(64));
    const original = session.intent;
    fetchApi.mockClear();
    await session.recover("b".repeat(64));
    expect(session.intent).toBe(original);
    expect(session.observationRequired).toBe(true);
    expect(session.message).not.toContain(failure);
    expect(session.message).not.toContain("private-password");
    expect(session.message).not.toContain("b".repeat(64));
    expect(fetchApi).not.toHaveBeenCalled();
    expect(entries.size).toBe(0);
  },
);

it.each(["scope", "unavailable", "malformed", "network"])(
  "does not apply a recovered tuple when current public status fails: %s",
  async (failure) => {
    host.native = false;
    const { WebSetupCreate } = await import("./setupClient.svelte");
    const session = new WebSetupCreate(recoveryHost);
    host.recoverIntent.mockResolvedValue(retainedCreate);
    fetchApi.mockImplementationOnce(async () =>
      Response.json(recoveryHost.status),
    );
    await session.recover("a".repeat(64));
    const original = session.intent;
    const status = session.status;
    if (failure === "network")
      fetchApi.mockRejectedValueOnce(new Error("private network detail"));
    else
      fetchApi.mockResolvedValueOnce(
        Response.json(
          failure === "scope"
            ? { ...recoveryHost.status, owners: ["idp"] }
            : failure === "unavailable"
              ? { ...recoveryHost.status, state: "unavailable" }
              : {
                  stage: "installation",
                  state: "creating",
                  ready: false,
                  owners: ["idp", "idp", "storage"],
                  secret: "private-value",
                },
          { status: failure === "unavailable" ? 503 : 200 },
        ),
      );
    await session.recover("b".repeat(64));
    expect(session.intent).toBe(original);
    expect(session.status).toBe(status);
    expect(session.observationRequired).toBe(true);
    expect(session.message).not.toContain("private");
    expect(host.submitCreate).not.toHaveBeenCalled();
    expect(host.cancelCreate).not.toHaveBeenCalled();
  },
);

it("does not retain a newly recovered tuple before current owner evidence is verified", async () => {
  host.native = false;
  const { WebSetupCreate } = await import("./setupClient.svelte");
  const session = new WebSetupCreate(recoveryHost);
  host.recoverIntent.mockResolvedValue(retainedCreate);
  fetchApi.mockResolvedValue(
    Response.json({ ...recoveryHost.status, owners: ["idp"] }),
  );
  await session.recover("a".repeat(64));
  expect(session.intent).toBeNull();
  expect(session.observationRequired).toBe(true);
});

it.each(["lookup", "status"] as const)(
  "serializes recovery with other requests and ignores late destruction during %s",
  async (phase) => {
    host.native = false;
    const { WebSetupCreate } = await import("./setupClient.svelte");
    const session = new WebSetupCreate(recoveryHost);
    let finishLookup!: (intent: SetupCreateIntent) => void;
    let finishStatus!: (response: Response) => void;
    host.recoverIntent.mockImplementation(
      () =>
        new Promise<SetupCreateIntent>((resolve) => {
          finishLookup = resolve;
        }),
    );
    fetchApi.mockImplementation(
      () =>
        new Promise<Response>((resolve) => {
          finishStatus = resolve;
        }),
    );
    const pending = session.recover("a".repeat(64));
    if (phase === "status") {
      finishLookup(retainedCreate);
      await vi.waitFor(() => expect(fetchApi).toHaveBeenCalledTimes(1));
    }
    expect(session.busy).toBe(true);
    await session.recover("b".repeat(64));
    await session.observe();
    await session.submit("b".repeat(64), "admin", "node", "password");
    await session.cancel("b".repeat(64));
    expect(host.recoverIntent).toHaveBeenCalledTimes(1);
    expect(fetchApi).toHaveBeenCalledTimes(phase === "status" ? 1 : 0);
    expect(host.submitCreate).not.toHaveBeenCalled();
    expect(host.cancelCreate).not.toHaveBeenCalled();
    const status = session.status;
    session.destroy();
    if (phase === "lookup") finishLookup(retainedCreate);
    else
      finishStatus(
        Response.json({ ...recoveryHost.status, state: "creating" }),
      );
    await pending;
    expect(session.intent).toBeNull();
    expect(session.status).toBe(status);
    expect(session.message).toBe("");
    expect(session.busy).toBe(false);
    await session.recover("a".repeat(64));
    expect(host.recoverIntent).toHaveBeenCalledTimes(1);
    expect(fetchApi).toHaveBeenCalledTimes(phase === "status" ? 1 : 0);
  },
);

it.each(["native", "independent", "invalid code", "insecure origin"])(
  "never sends recovery lookup outside authorized unified web setup: %s",
  async (failure) => {
    host.native = failure === "native";
    const { WebSetupCreate } = await import("./setupClient.svelte");
    const session = new WebSetupCreate(
      failure === "independent"
        ? {
            ...recoveryHost,
            status: { ...recoveryHost.status, owners: ["idp"] },
          }
        : recoveryHost,
    );
    if (failure === "insecure origin")
      vi.stubGlobal("window", {
        location: new URL("http://app.test"),
        localStorage: storage,
      });
    const random = vi.spyOn(crypto, "getRandomValues");
    await session.recover(
      failure === "invalid code" ? "invalid" : "a".repeat(64),
    );
    expect(session.intent).toBeNull();
    expect(host.recoverIntent).not.toHaveBeenCalled();
    expect(fetchApi).not.toHaveBeenCalled();
    expect(host.invoke).not.toHaveBeenCalled();
    expect(random).not.toHaveBeenCalled();
    expect(entries.size).toBe(0);
  },
);

it("uses the actual current HTTPS UI origin rather than PUBLIC_URL", async () => {
  host.native = false;
  host.authority = "https://node.test/idp";
  vi.stubGlobal("window", {
    location: new URL("http://app.test"),
    localStorage: storage,
  });
  const { getWebSetupHosts } = await import("./setupClient.svelte");
  await expect(getWebSetupHosts()).rejects.toThrow("current HTTPS UI origin");
  expect(fetchApi).not.toHaveBeenCalled();
  expect(host.invoke).not.toHaveBeenCalled();
});

it("discovers independent public login Client from IdP facts only after matching current owner readiness", async () => {
  host.native = false;
  host.authority = "https://node.test/idp";
  host.management = "https://policy.test/api";
  const operation = { purpose: "create", operation_nonce: "1".repeat(32), administrator: "Alice", node: "Node", idp: host.authority, management: host.management, storage: "https://data.test/api", issuer: host.authority, peer: null };
  let conflict = false;
  fetchApi.mockImplementation(async (url: string) => {
    const owner = url.startsWith(operation.idp) ? "idp" : url.startsWith(operation.management) ? "management" : "storage";
    if (url.endsWith("/status")) return Response.json({ stage: "installation", state: "ready", ready: true, owners: [owner] });
    return Response.json({ owner, operation: { ...operation, operation_nonce: conflict && owner === "storage" ? "2".repeat(32) : operation.operation_nonce }, facts: owner === "idp" ? { login_client: "01900000-0000-7000-8000-000000000123" } : {} });
  });
  const { ensureOidcClientId } = await import("./oidc.svelte");
  await expect(ensureOidcClientId()).resolves.toBe("01900000-0000-7000-8000-000000000123");
  conflict = true;
  await expect(ensureOidcClientId()).rejects.toThrow(/disagree/);
  expect(host.invoke).not.toHaveBeenCalled();
});
