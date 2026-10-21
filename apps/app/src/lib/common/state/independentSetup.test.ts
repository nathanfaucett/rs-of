import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SetupHost, SetupOwner } from "./setupApi";

const bases = {
  idp: "https://target.test/idp",
  management: "https://target.test/management",
  storage: "https://data.test/storage",
};
const owners: SetupOwner[] = ["idp", "management", "storage"];
const codes = {
  idp: "a".repeat(64),
  management: "b".repeat(64),
  storage: "c".repeat(64),
};
const create = {
  purpose: "create" as const,
  operation_nonce: "1".repeat(32),
  administrator: "Alice",
  node: "My Node",
  ...bases,
  issuer: bases.idp,
  peer: null,
};
const fetchApi = vi.fn();
function status(owner: SetupOwner, state = "awaiting_choice") {
  return {
    stage: "installation",
    state,
    ready: state === "ready",
    owners: [owner],
  };
}
function hosts(state = "awaiting_choice"): SetupHost[] {
  return owners.map((owner) => ({
    base: bases[owner],
    status: {
      ...status(owner, state),
      stage: state === "ready" ? "ready" : "installation",
    } as SetupHost["status"],
  }));
}
beforeEach(() => {
  fetchApi.mockReset();
  vi.stubGlobal("fetch", fetchApi);
  vi.stubGlobal("window", { location: new URL("https://app.test") });
});
afterEach(() => vi.unstubAllGlobals());

it("sends one exact Create tuple with each owner's own header and a password only to IdP", async () => {
  const api = await import("./independentSetupApi");
  fetchApi.mockResolvedValue(new Response(null, { status: 202 }));
  for (const owner of owners)
    await api.submitIndependentSetup(
      bases[owner],
      owner,
      codes[owner],
      create,
      "protected-password",
  );
  expect(
    fetchApi.mock.calls.map(([url, init]) => [
      url,
      init.headers.Authorization,
      JSON.parse(init.body),
    ]),
  ).toEqual(
    owners.map((owner) => [
      bases[owner] + "/independent/setup/create",
      "Setup " + codes[owner],
      {
        ...create,
        ...(owner === "idp" ? { password: "protected-password" } : {}),
      },
    ]),
  );
  for (const [, init] of fetchApi.mock.calls)
    expect(init).toMatchObject({
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
    });
});

it("discovers ready independent IdP login facts only after all exact operations and issuers agree", async () => {
  const api = await import("./setupApi");
  fetchApi.mockImplementation(async (url: string) => {
    if (url === bases.idp + "/setup/status") return new Response(null, { status: 404 });
    const owner = owners.find((owner) => url.startsWith(bases[owner] + "/"))!;
    return url.endsWith("/independent/setup/status")
      ? Response.json(status(owner, "ready"))
      : Response.json({
          operation: create,
          owner,
          clients: [],
          registered: [],
          facts:
            owner === "idp"
              ? {
                  login_client: "01900000-0000-7000-8000-000000000001",
                }
              : {},
        });
  });
  const current = await api.fetchSetupHosts(bases);
  expect(current.every((host) => host.status.ready)).toBe(true);
  expect(current[0].status.login_client?.client_id).toBe(
    "01900000-0000-7000-8000-000000000001",
  );
  fetchApi.mockImplementation(async (url: string) => {
    if (url === bases.idp + "/setup/status") return new Response(null, { status: 404 });
    const owner = owners.find((owner) => url.startsWith(bases[owner] + "/"))!;
    return url.endsWith("/independent/setup/status")
      ? Response.json(status(owner, "ready"))
      : Response.json({
          operation: {
            ...create,
            operation_nonce:
              owner === "storage" ? "2".repeat(32) : create.operation_nonce,
          },
          owner,
          facts: {},
        });
  });
  await expect(api.fetchSetupHosts(bases)).rejects.toThrow(/disagree/);
});

it("retains a single independent Join across partial admission and reload recovery without passwords or nonce replacement", async () => {
  const { IndependentSetupSession } = await import("./independentSetup.svelte");
  const bound: Record<string, object> = {};
  let refuseManagement = true;
  const source = "https://source.test/management#" + "d".repeat(64);
  fetchApi.mockImplementation(async (url: string, init: RequestInit = {}) => {
    const owner = owners.find((owner) => url.startsWith(bases[owner] + "/"))!;
    if (url.endsWith("/status"))
      return Response.json(
        status(owner, bound[owner] ? "unfinished" : "awaiting_choice"),
      );
    if (url.endsWith("/intent"))
      return bound[owner]
        ? Response.json(bound[owner])
        : new Response(null, { status: 204 });
    if (url.endsWith("/join")) {
      if (owner === "management" && refuseManagement)
        return new Response(null, { status: 409 });
      bound[owner] = {
        ...JSON.parse(init.body as string),
        issuer: "https://canonical.test/idp",
      };
      return new Response(null, { status: 202 });
    }
    return new Response(null, { status: 409 });
  });
  const session = new IndependentSetupSession(hosts());
  await session.submit(codes, {
    purpose: "join",
    node: "Joining Node",
    peer: source,
  });
  const sent = fetchApi.mock.calls.filter(([url]) => url.endsWith("/join"));
  expect(sent).toHaveLength(3);
  for (const [, init] of sent)
    expect(JSON.parse(init.body)).toMatchObject({
      purpose: "join",
      administrator: "",
      issuer: "",
      peer: source,
    });
  expect(sent.every(([, init]) => !("password" in JSON.parse(init.body)))).toBe(
    true,
  );
  expect(session.observationRequired).toBe(true);
  const nonce = session.intent!.operation_nonce;
  session.destroy();
  const reload = new IndependentSetupSession(hosts("unfinished"));
  await reload.recover(codes);
  expect(reload.intent?.operation_nonce).toBe(nonce);
  expect(reload.intent?.issuer).toBe("https://canonical.test/idp");
  expect(reload.observationRequired).toBe(false);
  refuseManagement = false;
  await reload.submit(codes, {
    purpose: "join",
    node: "Joining Node",
    peer: source,
  });
  expect(
    Object.values(bound).every(
      (value) => (value as typeof create).operation_nonce === nonce,
    ),
  ).toBe(true);
  expect(JSON.stringify(reload)).not.toContain(codes.idp);
  expect(JSON.stringify(reload)).not.toContain("password");
});

it("does not adopt a recovered tuple if completion switches to a different operation during status revalidation", async () => {
  const { IndependentSetupSession } = await import("./independentSetup.svelte");
  let lookups = 0;
  fetchApi.mockImplementation(async (url: string) => {
    if (url === bases.idp + "/setup/status") return new Response(null, { status: 404 });
    const owner = owners.find((owner) => url.startsWith(bases[owner] + "/"))!;
    if (url.endsWith("/status"))
      return Response.json(
        status(owner, lookups === 3 ? "ready" : "unfinished"),
      );
    if (url.endsWith("/intent")) {
      lookups++;
      return Response.json(create);
    }
    return Response.json({
      owner,
      operation: { ...create, operation_nonce: "2".repeat(32) },
      facts: {},
    });
  });
  const session = new IndependentSetupSession(hosts("unfinished"));
  await session.recover(codes);
  expect(session.ready).toBe(false);
  expect(session.intent).toBeNull();
  expect(session.observationRequired).toBe(true);
});

it.each([
  "operation_nonce",
  "node",
  "issuer",
  "peer",
  "idp",
  "management",
  "storage",
  "purpose",
])(
  "refuses recovered %s substitutions without replacing an existing exact Join or resuming mutation",
  async (field) => {
    const { IndependentSetupSession } = await import(
      "./independentSetup.svelte"
    );
    const join = {
      ...create,
      purpose: "join",
      administrator: "",
      issuer: "https://canonical.test/idp",
      peer: "https://source.test/management#" + "d".repeat(64),
    };
  let corrupt = false;
  fetchApi.mockImplementation(async (url: string) => {
      const owner = owners.find((owner) => url.startsWith(bases[owner] + "/"))!;
      if (url.endsWith("/status"))
        return Response.json(status(owner, "unfinished"));
    let operation = join;
      if (corrupt && owner === "storage")
        operation = {
          ...join,
          [field]:
            field === "operation_nonce"
              ? "2".repeat(32)
              : field === "node"
                ? "Other Node"
                : field === "purpose"
                  ? "create"
                  : field === "peer"
                    ? "https://other.test/management#" + "e".repeat(64)
                    : "https://other.test/api",
        };
    return Response.json(operation);
  });
  const session = new IndependentSetupSession(hosts("unfinished"));
  await session.recover(codes);
  const exact = session.intent;
  expect(exact).toMatchObject(join);
  corrupt = true;
  await session.recover(codes);
  expect(session.intent).toBe(exact);
  expect(session.observationRequired).toBe(true);
    expect(
      fetchApi.mock.calls.every(
        ([url]) => url.endsWith("/status") || url.endsWith("/intent"),
      ),
    ).toBe(true);
  },
);

it("cancels recovered exact tuples per owner with fresh codes and preserves partial cancellation uncertainty", async () => {
  const { IndependentSetupSession } = await import("./independentSetup.svelte");
  fetchApi.mockImplementation(async (url: string) => {
    if (url === bases.idp + "/setup/status") return new Response(null, { status: 404 });
    const owner = owners.find((owner) => url.startsWith(bases[owner] + "/"))!;
    if (url.endsWith("/status"))
      return Response.json(status(owner, "unfinished"));
    if (url.endsWith("/intent")) return Response.json(create);
    return new Response(null, {
      status: owner === "management" ? 409 : 204,
    });
  });
  const session = new IndependentSetupSession(hosts("unfinished"));
  await session.recover(codes);
  const exact = session.intent;
  await session.cancel(codes);
  const requests = fetchApi.mock.calls.filter(([url]) =>
    url.endsWith("/cancel"),
  );
  expect(
    requests.map(([url, init]) => [
      url,
      init.headers.Authorization,
      JSON.parse(init.body),
    ]),
  ).toEqual(
    owners.map((owner) => [
      bases[owner] + "/independent/setup/cancel",
      "Setup " + codes[owner],
      create,
    ]),
  );
  expect(session.outcomes.management).toContain("uncertain");
  expect(session.observationRequired).toBe(true);
  expect(session.ready).toBe(false);
  expect(session.intent).toBe(exact);
});

it("requires all current owner scopes and codes before producing a fresh nonce or any mutation", async () => {
  const { IndependentSetupSession } = await import("./independentSetup.svelte");
  const randomness = vi.fn();
  vi.stubGlobal("crypto", { getRandomValues: randomness });
  fetchApi.mockImplementation(async (url: string) =>
    Response.json(
      status(owners.find((owner) => url.startsWith(bases[owner] + "/"))!),
    ),
  );
  const session = new IndependentSetupSession(hosts());
  await session.submit(
    { ...codes, storage: "" },
    { purpose: "create", administrator: "Alice", node: "Node" },
    "protected-password",
  );
  expect(session.intent).toBeNull();
  expect(randomness).not.toHaveBeenCalled();
  expect(fetchApi.mock.calls.every(([url]) => url.endsWith("/status"))).toBe(
    true,
  );
  fetchApi.mockImplementation(async () => Response.json(status("idp")));
  await session.observe();
  expect(session.observationRequired).toBe(true);
  expect(session.ready).toBe(false);
});

it("recovers an entirely unbound operation without inventing an intent or nonce", async () => {
  const { IndependentSetupSession } = await import("./independentSetup.svelte");
  const randomness = vi.fn();
  vi.stubGlobal("crypto", { getRandomValues: randomness });
  fetchApi.mockImplementation(async (url: string) =>
    url.endsWith("/status")
      ? Response.json(
          status(owners.find((owner) => url.startsWith(bases[owner] + "/"))!),
        )
      : new Response(null, { status: 204 }),
  );
  const session = new IndependentSetupSession(hosts());
  await session.recover(codes);
  expect(session.intent).toBeNull();
  expect(session.observationRequired).toBe(false);
  expect(randomness).not.toHaveBeenCalled();
});

it("never treats three unrelated ready flags as global readiness or sends retired codes to ready owners", async () => {
  const { IndependentSetupSession } = await import("./independentSetup.svelte");
  fetchApi.mockImplementation(async (url: string) => {
    if (url === bases.idp + "/setup/status") return new Response(null, { status: 404 });
    const owner = owners.find((owner) => url.startsWith(bases[owner] + "/"))!;
    if (url.endsWith("/status")) return Response.json(status(owner, "ready"));
    return Response.json({
      owner,
      operation: {
        ...create,
        operation_nonce:
          owner === "storage" ? "2".repeat(32) : create.operation_nonce,
      },
      facts: {},
    });
  });
  const session = new IndependentSetupSession(hosts("ready"));
  await session.recover(codes);
  expect(session.ready).toBe(false);
  expect(
    fetchApi.mock.calls.every(([, init]) => !init.headers?.Authorization),
  ).toBe(true);
});
