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
beforeEach(() => {
  vi.resetModules();
  fetch.mockReset();
  fetchSetupHosts.mockClear();
  selected.idp = `${origin}/idp`;
  selected.storage = `${origin}/storage`;
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
