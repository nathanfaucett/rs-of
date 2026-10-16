import { isHttpError } from "@sveltejs/kit";
import { render } from "svelte/server";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SetupHost } from "$lib/common/state/setupApi";

const host = vi.hoisted(() => ({
  native: true,
  invoke: vi.fn(),
  fetch: vi.fn(),
  management: "https://policy.test/api" as string | null,
  oidc: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => host.native,
  invoke: host.invoke,
}));

vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
vi.mock("$lib/common/state/oidc.svelte", () => ({ getOidcClient: host.oidc }));
vi.mock("$lib/common/state/idpClient.svelte", () => ({
  getIdpApiUrl: () => "https://node.test/idp",
}));

vi.mock("$lib/common/state/managementClient.svelte", () => ({
  getManagementApiUrl: () => host.management,
}));
vi.mock("$env/dynamic/public", () => ({
  env: { PUBLIC_STORAGE_BASE_URL: "https://data.test/api" },
}));

const { load } = await import("./+page");
const { default: Installation } = await import("./+page.svelte");

it.each([
  [
    "ready",
    "/signin",
    "Local completion does not sign you in or grant permissions.",
  ],
  ["device", "/setup/device", "Continue Device setup"],
])(
  "shows %s completion navigation without recreating the Installation",
  (stage, href, message) => {
    const { body } = render(Installation, { props: { data: { stage } } });
    expect(body).toContain(`href="${href}"`);
    expect(body).toContain(message);
    expect(body).not.toContain("<form");
    expect(body).not.toContain("Create local Node");
    expect(body).not.toContain("Request approval and join");
    expect(body).toContain("Type RESET to confirm");
    expect(body).toMatch(
      /<button[^>]*disabled[^>]*>Reset \/ retry pending reset<\/button>/,
    );
    expect(host.invoke).not.toHaveBeenCalled();
  },
);

it("offers automatic desktop Create setup without issuer or Client configuration", () => {
  const { body } = render(Installation, {
    props: { data: { stage: "installation" } },
  });
  expect(body).toContain("Create an Installation");
  expect(body).toContain("Join through a trusted peer");
  expect(body).toContain("Administrator name");
  expect(body).toContain("Password");
  expect(body).not.toContain("Canonical Installation issuer");
  expect(body).not.toContain("Client secret");
  expect(body).not.toContain("Client ID");
  expect(body).not.toContain("Recover retained Create");
  expect(host.invoke).not.toHaveBeenCalled();
});

it("offers unified web Create with protected credentials and explicit observation without native reset controls", () => {
  host.native = false;
  const { body } = render(Installation, {
    props: {
      data: {
        stage: "installation",
        hosts: [
          {
            base: "https://node.test/idp",
            status: {
              stage: "installation",
              state: "awaiting_choice",
              ready: false,
              owners: ["idp", "management", "storage"],
            },
          },
        ],
      },
    },
  });
  expect(body).toContain("Create an Installation");
  expect(body).toContain("Join through a trusted peer");
  expect(body).toContain("Unified web Create and Join controls are available");
  expect(body).toContain(
    "Independent-owner and native API Join remain unfinished",
  );
  expect(body).toContain("source Node");
  expect(body).toContain("Administrator name");
  expect(body).toContain("Node name");
  expect(body).toContain("Password");
  expect(body).toContain("Observe host status");
  expect(body).toContain("Recover retained Create");
  expect(body).toMatch(/recover the\s+exact intent using the host code/);
  expect(body).toContain("not membership, domain permissions or readiness");
  expect(body).toMatch(/<button[^>]*disabled[^>]*>\s*Create Node\s*<\/button>/);
  expect(body).toContain("Host setup code");
  expect(body).toContain("Verify setup code");
  expect(body).not.toContain(
    "Installation is available only in the desktop app.",
  );
  expect(body).not.toContain("Reset this local Node");
  expect(body).toContain("<form");
});

it.each([null, undefined, "unknown", "desktop-only", { stage: "ready" }])(
  "denies invalid local setup evidence %j rather than redirecting to completion",
  async (stage) => {
    host.invoke.mockResolvedValue(stage);
    const failure = await load().catch((cause: unknown) => cause);
    expect(isHttpError(failure)).toBe(true);
    expect(failure).toMatchObject({ status: 503 });
  },
);

it.each(["installation", "unavailable"])(
  "preserves the existing %s setup state",
  async (stage) => {
    host.invoke.mockResolvedValue(stage);
    await expect(load()).resolves.toEqual({ stage });
    expect(host.invoke.mock.calls).toEqual([["get_setup_stage"]]);
  },
);

it("reads web setup evidence from the API without invoking native controls", async () => {
  host.native = false;
  host.fetch.mockResolvedValue(
    Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    }),
  );
  await expect(load()).resolves.toEqual({
    stage: "installation",
    hosts: [
      {
        base: "https://node.test/idp",
        status: {
          stage: "installation",
          state: "awaiting_choice",
          ready: false,
          owners: ["idp", "management", "storage"],
        },
      },
    ],
  });
  expect(host.invoke).not.toHaveBeenCalled();
  expect(host.fetch).toHaveBeenCalledWith(
    "https://node.test/idp/setup/status",
    {
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
    },
  );
});

it("does not turn a local evidence failure into readiness or a redirect", async () => {
  const failure = new Error("Local secure storage unavailable");
  host.invoke.mockRejectedValue(failure);
  await expect(load()).rejects.toBe(failure);
});

beforeEach(() => {
  host.native = true;
  host.management = "https://policy.test/api";
  vi.stubGlobal("window", { location: new URL("https://app.test") });
  host.invoke.mockReset();
  host.oidc.mockReset();
  host.fetch.mockReset();
  vi.stubGlobal("fetch", host.fetch);
});

afterEach(() => vi.unstubAllGlobals());

it("keeps a locally ready Node's Installation reset page reachable without signing in", async () => {
  host.invoke.mockResolvedValue("ready");
  await expect(load()).resolves.toEqual({ stage: "ready" });
  expect(host.invoke.mock.calls).toEqual([["get_setup_stage"]]);
});

it("keeps Device completion reachable from the Installation reset page", async () => {
  host.invoke.mockResolvedValue("device");
  await expect(load()).resolves.toEqual({ stage: "device" });
  expect(host.invoke.mock.calls).toEqual([["get_setup_stage"]]);
});

it("shows three independent host scopes and API bases with no native controls", () => {
  host.native = false;
  const hosts: SetupHost[] = [
    {
      base: "https://node.test/idp",
      status: {
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners: ["idp"],
      },
    },
    {
      base: "https://node.test/policy",
      status: {
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners: ["management"],
      },
    },
    {
      base: "https://store.test/files",
      status: {
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners: ["storage"],
      },
    },
  ];
  const { body } = render(Installation, {
    props: { data: { stage: "installation", hosts } },
  });
  for (const host of hosts) expect(body).toContain(`value="${host.base}"`);
  expect(body).toMatch(/Independent\s+hosts require their own codes/);
  expect(body).not.toContain("Type RESET to confirm");
  expect(body).toContain("Independent Create unavailable");
  expect(body).not.toContain("Administrator name");
  expect(body).not.toContain("Node name");
  expect(body).not.toContain("Recover retained Create");
  expect(host.invoke).not.toHaveBeenCalled();
});

it.each<{ hosts?: SetupHost[] }>([
  { hosts: undefined },
  {
    hosts: [
      {
        base: "https://node.test/idp",
        status: {
          stage: "unavailable",
          state: "unavailable",
          ready: false,
          owners: ["idp", "management", "storage"],
        },
      },
    ],
  },
])(
  "does not offer code submission or completion with missing/unhealthy host evidence",
  ({ hosts }) => {
    host.native = false;
    const { body } = render(Installation, {
      props: { data: { stage: "unavailable", hosts } },
    });
    expect(body).toContain('role="alert"');
    expect(body).not.toContain('type="password"');
    expect(body).not.toContain('href="/signin"');
    expect(body).not.toContain('href="/setup/device"');
    expect(host.invoke).not.toHaveBeenCalled();
  },
);

it("reports mismatched independent web owner evidence as a visible 503, not completion", async () => {
  host.native = false;
  host.fetch.mockImplementation(async () =>
    Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp"],
    }),
  );
  const failure = await load().catch((cause: unknown) => cause);
  expect(isHttpError(failure)).toBe(true);
  expect(failure).toMatchObject({
    status: 503,
    body: {
      message: "Setup scope does not match the configured management service",
    },
  });
  expect(host.fetch.mock.calls.map(([url]) => url)).toEqual([
    "https://node.test/idp/setup/status",
    "https://policy.test/api/setup/status",
  ]);
  expect(host.invoke).not.toHaveBeenCalled();
});

it("routes fresh unified web setup to Installation, never normal OAuth/sign-in completion", async () => {
  host.native = false;
  host.fetch.mockResolvedValue(
    Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    }),
  );
  const { load: routeSetup } = await import("../+page");
  await expect(routeSetup()).rejects.toMatchObject({
    status: 302,
    location: "/setup/installation",
  });
  expect(host.fetch.mock.calls.map(([url]) => url)).toEqual([
    "https://node.test/idp/setup/status",
  ]);
  expect(host.invoke).not.toHaveBeenCalled();
  expect(host.oidc).not.toHaveBeenCalled();
});

it.each(["creating", "joining", "unfinished"] as const)(
  "observes %s after reload without manufacturing a reconnect intent",
  (state) => {
    host.native = false;
    const { body } = render(Installation, {
      props: {
        data: {
          stage: "installation",
          hosts: [
            {
              base: "https://node.test/idp",
              status: {
                stage: "installation",
                state,
                ready: false,
                owners: ["idp", "management", "storage"],
              },
            },
          ],
        },
      },
    });
    expect(body).toContain("Observe host status");
    expect(body).toContain("Recover retained Create");
    expect(body).toMatch(
      /Recover the exact retained intent\s+before\s+retrying or canceling/,
    );
    expect(body).not.toContain("Retained setup cannot be resumed");
    expect(body).not.toContain("reload cannot recover");
    expect(body).not.toContain("Administrator name");
    expect(body).not.toContain("Create Node</button>");
    expect(body).not.toContain("Cancel exact Create");
    expect(body).not.toContain('href="/signin"');
    expect(host.invoke).not.toHaveBeenCalled();
    expect(host.fetch).not.toHaveBeenCalled();
  },
);

it("shows verified unified web readiness without automatic sign-in or code/password entry", () => {
  host.native = false;
  const { body } = render(Installation, {
    props: {
      data: {
        stage: "ready",
        hosts: [
          {
            base: "https://node.test/idp",
            status: {
              stage: "ready",
              state: "ready",
              ready: true,
              owners: ["idp", "management", "storage"],
              login_client: { client_id: "public-login", scopes: ["openid"] },
            },
          },
        ],
      },
    },
  });
  expect(body).toContain('href="/signin"');
  expect(body).toContain("Node readiness verified");
  expect(body).not.toContain('type="password"');
  expect(body).not.toContain("Recover retained Create");
  expect(host.oidc).not.toHaveBeenCalled();
  expect(host.invoke).not.toHaveBeenCalled();
});

it("does not accept invented web readiness or send the operator through OAuth on load failure", async () => {
  host.native = false;
  host.fetch.mockResolvedValue(
    Response.json({
      stage: "ready",
      state: "ready",
      ready: true,
      owners: ["idp", "management", "storage"],
    }),
  );
  const { load: routeSetup } = await import("../+page");
  await expect(routeSetup()).rejects.toMatchObject({
    status: 503,
    body: { message: "Setup evidence is invalid" },
  });
  expect(host.invoke).not.toHaveBeenCalled();
  expect(host.oidc).not.toHaveBeenCalled();
});
