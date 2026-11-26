import { beforeEach, expect, it, vi } from "vitest";

const origin = "https://node.test";
const code = "a".repeat(64);
const selected = vi.hoisted(() => ({ idp: "", storage: "" }));
const fetchSetupHosts = vi.hoisted(() => vi.fn());
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_UNIFIED_ORIGIN: origin } }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => false }));
vi.mock("./idpClient.svelte", () => ({ getIdpApiUrl: () => selected.idp }));
vi.mock("./deviceClient.svelte", () => ({ getDeviceApiUrl: () => `${origin}/device` }));
vi.mock("./storageApiUrl.svelte", () => ({ getStorageApiUrl: () => selected.storage }));
vi.mock("./setupApi", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./setupApi")>()),
  fetchSetupHosts,
}));

const fetch = vi.fn<typeof globalThis.fetch>();

class FakeEventSource {
  static instances: FakeEventSource[] = [];
  onmessage?: (event: MessageEvent<string>) => void;
  onerror?: () => void;
  close = vi.fn();

  readonly url: string;
  readonly options: EventSourceInit | undefined;

  constructor(url: string, options?: EventSourceInit) {
    this.url = url;
    this.options = options;
    FakeEventSource.instances.push(this);
  }

  static at(index: number) {
    const source = FakeEventSource.instances[index];
    if (!source) throw new Error(`No EventSource at index ${index}`);
    return source;
  }

  message(descriptor: unknown) {
    this.onmessage?.(new MessageEvent("message", { data: JSON.stringify(descriptor) }));
  }
}
beforeEach(() => {
  vi.resetModules();
  fetch.mockReset();
  fetchSetupHosts.mockClear();
  selected.idp = `${origin}/idp`;
  selected.storage = `${origin}/storage`;
  FakeEventSource.instances = [];
  vi.stubGlobal("EventSource", FakeEventSource);
  vi.stubGlobal("fetch", fetch);
  vi.stubGlobal("window", { location: new URL("https://app.test") });
});

it.each(["create", "join"] as const)(
  "hands the code to %s only after successful choice",
  async (choice) => {
    const setup = await import("./nodeSetup");
    fetch.mockResolvedValueOnce(new Response(null, { status: 401 }));
    await expect(setup.chooseNodeSetup(choice, code)).rejects.toThrow("not accepted");
    expect(setup.nodeSetupCode(choice)).toBe("");

    fetch.mockResolvedValueOnce(new Response(null, { status: 204 }));
    await setup.chooseNodeSetup(choice, code);
    expect(setup.nodeSetupCode(choice)).toBe(code);
    expect(setup.nodeSetupCode(choice === "create" ? "join" : "create")).toBe("");
    selected.idp = "https://other.test/idp";
    expect(setup.nodeSetupCode(choice)).toBe("");
    selected.idp = `${origin}/idp`;
    vi.resetModules();
    expect((await import("./nodeSetup")).nodeSetupCode(choice)).toBe("");
  }
);

it("reads protected recovery and sends only the exact Create and cancel fields", async () => {
  const setup = await import("./nodeSetup");
  const intent = { administrator: "admin", password_required: false, operation_nonce: "nonce" };
  fetch
    .mockResolvedValueOnce(new Response(null, { status: 204 }))
    .mockResolvedValueOnce(Response.json(intent))
    .mockResolvedValueOnce(new Response(null, { status: 202 }))
    .mockResolvedValueOnce(new Response(null, { status: 202 }))
    .mockResolvedValueOnce(new Response(null, { status: 204 }));

  expect(await setup.readNodeCreateIntent(code)).toBeNull();
  expect(await setup.readNodeCreateIntent(code)).toEqual(intent);
  await setup.createNode(code, "admin", "original password");
  await setup.createNode(code, intent.administrator);
  await setup.cancelNodeCreate(code, intent.operation_nonce);

  expect(fetch.mock.calls.map(([url, init]) => [url, init?.body])).toEqual([
    [`${origin}/setup/node/create/intent`, undefined],
    [`${origin}/setup/node/create/intent`, undefined],
    [`${origin}/setup/node/create`, '{"administrator":"admin","password":"original password"}'],
    [`${origin}/setup/node/create`, '{"administrator":"admin"}'],
    [`${origin}/setup/node/create/cancel`, '{"operation_nonce":"nonce"}'],
  ]);
  for (const [, init] of fetch.mock.calls) {
    expect(new Headers(init?.headers).get("Authorization")).toBe(`Setup ${code}`);
    expect(init).toMatchObject({
      method: "POST",
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
    });
  }
  expect(new Headers(fetch.mock.calls[0]?.[1]?.headers).has("Content-Type")).toBe(false);
});

it("uses the same descriptor for status snapshots and default SSE messages", async () => {
  const setup = await import("./nodeSetup");
  const update = vi.fn();
  const disconnect = vi.fn();
  const close = setup.watchNodeSetupStatus(update, disconnect);
  const source = FakeEventSource.at(0);
  expect(source.url).toBe(`${origin}/setup/node/events`);
  expect(source.options).toBeUndefined();

  for (const [descriptor, status] of [
    [{ state: "starting" }, { route: "/setup", state: "starting" }],
    [{ state: "creating" }, { route: "/setup/installation", state: "creating", active: false }],
    [
      { state: "creating", active: true },
      { route: "/setup/installation", state: "creating", active: true },
    ],
    [{ state: "blocked" }, { route: "/setup", state: "blocked" }],
    [
      {
        state: "ready",
        idp: selected.idp,
        storage: selected.storage,
        login_client: { client_id: "node-login", scopes: ["openid"] },
      },
      {
        route: "/signin",
        state: "ready",
        idp: selected.idp,
        storage: selected.storage,
        loginClient: { client_id: "node-login", scopes: ["openid"] },
      },
    ],
  ]) {
    fetch.mockResolvedValueOnce(Response.json(descriptor));
    expect(await setup.nodeSetupStatus()).toEqual(status);
    source.message(descriptor);
    expect(update).toHaveBeenLastCalledWith(status);
  }
  expect(disconnect).not.toHaveBeenCalled();
  close();
  source.message({ state: "starting" });
  source.onerror?.();
  expect(update).toHaveBeenCalledTimes(5);
  expect(disconnect).not.toHaveBeenCalled();
  expect(source.close).toHaveBeenCalledOnce();
});

it("closes on error and resumes only when a caller opens a new stream", async () => {
  const { watchNodeSetupStatus } = await import("./nodeSetup");
  const update = vi.fn();
  const disconnect = vi.fn();
  const close = watchNodeSetupStatus(update, disconnect);
  const source = FakeEventSource.at(0);

  source.onerror?.();
  source.onerror?.();
  source.message({ state: "creating", active: true });
  close();
  expect(source.close).toHaveBeenCalledOnce();
  expect(disconnect).toHaveBeenCalledOnce();
  expect(update).not.toHaveBeenCalled();
  expect(FakeEventSource.instances).toHaveLength(1);
  expect(fetch).not.toHaveBeenCalled();

  const closeResumed = watchNodeSetupStatus(update, disconnect);
  const resumed = FakeEventSource.at(1);
  resumed.message({ state: "creating", active: true });
  expect(update).toHaveBeenCalledWith({
    route: "/setup/installation",
    state: "creating",
    active: true,
  });
  closeResumed();
  expect(resumed.close).toHaveBeenCalledOnce();
});

it("closes instead of reporting readiness from an invalid descriptor", async () => {
  const { watchNodeSetupStatus } = await import("./nodeSetup");
  const update = vi.fn();
  const disconnect = vi.fn();
  watchNodeSetupStatus(update, disconnect);
  const source = FakeEventSource.at(0);
  source.message({
    state: "ready",
    idp: selected.idp,
    storage: selected.storage,
  });
  expect(source.close).toHaveBeenCalledOnce();
  expect(disconnect).toHaveBeenCalledOnce();
  expect(update).not.toHaveBeenCalled();
});

it("uses only the Node descriptor to verify local readiness and selected bases", async () => {
  const { observeOnboarding } = await import("./onboarding");
  const ready = { state: "ready", idp: selected.idp, storage: selected.storage };
  const login_client = { client_id: "node-login", scopes: ["openid"] };
  fetch.mockImplementation(async () => Response.json({ ...ready, login_client }));
  expect(await observeOnboarding()).toMatchObject({ state: "ready", loginClient: login_client });
  fetch.mockResolvedValueOnce(Response.json(ready));
  expect(await observeOnboarding()).toMatchObject({ state: "blocked" });
  selected.storage = "https://other.test/storage";
  expect(await observeOnboarding()).toMatchObject({ state: "blocked", action: "prerequisite" });
  expect(fetchSetupHosts).not.toHaveBeenCalled();
  expect(fetch.mock.calls.map(([url]) => url)).toEqual(Array(3).fill(`${origin}/setup/node`));
});
