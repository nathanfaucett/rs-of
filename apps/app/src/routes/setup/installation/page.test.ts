import { isHttpError, isRedirect } from "@sveltejs/kit";
import { render } from "svelte/server";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SetupHost } from "$lib/common/state/setupApi";

const host = vi.hoisted(() => ({
  native: true,
  invoke: vi.fn(),
  fetch: vi.fn(),
  management: "https://policy.test/api" as string | null,
  idp: "https://node.test/idp" as string | null,
  oidc: vi.fn(),
  clientId: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => host.native,
  invoke: host.invoke,
}));

vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
vi.mock("$lib/common/state/oidc.svelte", () => ({
  getOidcClient: host.oidc,
  ensureOidcClientId: host.clientId,
}));
vi.mock("$lib/common/state/idpClient.svelte", () => ({
  getIdpApiUrl: () => host.idp,
  ensureTauriIdpApiUrl: async () => "https://node.test/idp",
  usesRemoteApis: () => !host.native,
}));

vi.mock("$lib/common/state/managementClient.svelte", () => ({
  getManagementApiUrl: () => host.management,
  ensureTauriManagementApiUrl: async () => {},
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
    const { body } = render(Installation, {
      props: { data: { stage, bundled: true } },
    });
    expect(body).toContain(`href="${href}"`);
    expect(body).toContain(message);
    expect(body).not.toContain("<form");
    expect(body).not.toContain("Create local Node");
    expect(body).not.toContain("Request approval and join");
    expect(body).toContain("Type RESET to confirm");
    expect(body).toMatch(
      /<button[^>]*disabled[^>]*>Reset bundled Node explicitly<\/button>/,
    );
    expect(host.invoke).not.toHaveBeenCalled();
  },
);

it("offers bundled Create through the same HTTPS controls and explicit protected local code retrieval", () => {
  const hosts: SetupHost[] = [
    {
      base: "https://node.test/idp",
      status: {
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners: ["idp", "management", "storage"],
      },
    },
  ];
  const { body } = render(Installation, {
    props: { data: { stage: "installation", bundled: true, hosts } },
  });
  expect(body).toContain("Create an Installation");
  expect(body).toContain("Get bundled host code");
  expect(body).toContain("Resume setup");
  expect(body).toContain("Refresh status");
  expect(body).toContain("Administrator name");
  expect(body).not.toContain("Create local Node");
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
  expect(body).not.toContain(
    "Unified API Create and Join controls are available",
  );
  expect(body).not.toContain("Setup host / exact API base");
  expect(body).not.toContain("Your deployment must install");
  expect(body).toMatch(
    /<details>\s*<summary>Recovery and diagnostics<\/summary>/,
  );
  expect(body).toMatch(/<details>\s*<summary>Connection details<\/summary>/);
  expect(body).toContain("Administrator name");
  expect(body).toContain("Node name");
  expect(body).toContain("Password");
  expect(body).toContain("Refresh status");
  expect(body).toContain("Resume setup");
  expect(body).toContain("Keep it private");
  expect(body).toMatch(/<button[^>]*disabled[^>]*>\s*Create Node\s*<\/button>/);
  expect(body).toContain("Host setup code");
  expect(body).toContain("Verify setup code");
  expect(body).not.toContain(
    "Installation is available only in the desktop app.",
  );
  expect(body).not.toContain("Reset this local Node");
  expect(body).toContain("<form");
});

function readyNative(stage: unknown) {
  host.invoke.mockImplementation(async (command) =>
    command === "get_native_host_error" ? null : stage,
  );
  host.fetch.mockImplementation(async () =>
    Response.json({
      stage: "installation",
      state: "ready",
      ready: true,
      owners: ["idp", "management", "storage"],
      login_client: {
        client_id: "01900000-0000-7000-8000-000000000001",
        scopes: ["openid"],
      },
    }),
  );
}
it.each([null, undefined, "unknown", "desktop-only", { stage: "ready" }])(
  "refuses invalid native completion observation %j after HTTPS readiness",
  async (stage) => {
    readyNative(stage);
    await expect(load()).resolves.toMatchObject({
      stage: "unavailable",
      bundled: true,
    });
  },
);
it("shows native host prerequisite failure without probing or inventing fresh setup", async () => {
  host.invoke.mockResolvedValue("OS trust installation refused");
  await expect(load()).resolves.toEqual({
    stage: "unavailable",
    bundled: true,
    nativeError: "OS trust installation refused",
  });
  expect(host.fetch).not.toHaveBeenCalled();
});

it.each([null, ""])(
  "routes missing web endpoint %j to configuration without setup probes",
  async (base) => {
    host.native = false;
    host.idp = base;
    const failure = await load().catch((cause: unknown) => cause);
    expect(isRedirect(failure)).toBe(true);
    expect(failure).toMatchObject({ status: 302, location: "/config" });
    expect(host.fetch).not.toHaveBeenCalled();
    expect(host.invoke).not.toHaveBeenCalled();
    expect(host.oidc).not.toHaveBeenCalled();
  },
);

it("keeps configured web setup transport failures as 503 instead of configuration or fresh setup", async () => {
  host.native = false;
  host.fetch.mockRejectedValue(new TypeError("Failed to fetch"));
  expect(isHttpError(await load().catch((cause: unknown) => cause), 503)).toBe(
    true,
  );
  expect(host.invoke).not.toHaveBeenCalled();
});

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
    expect.objectContaining({
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
    }),
  );
});

it("does not turn a local evidence failure into readiness or a redirect", async () => {
  const failure = new Error("Local secure storage unavailable");
  host.invoke.mockRejectedValue(failure);
  await expect(load()).resolves.toMatchObject({
    stage: "unavailable",
    bundled: true,
    nativeError: expect.stringContaining("observation unavailable"),
  });
  expect(host.fetch).not.toHaveBeenCalled();
});

beforeEach(() => {
  host.native = true;
  host.management = "https://policy.test/api";
  host.idp = "https://node.test/idp";
  host.clientId.mockReset().mockResolvedValue("verified-client");
  vi.stubGlobal("window", { location: new URL("https://app.test") });
  host.invoke.mockReset().mockResolvedValue(null);
  host.oidc.mockReset();
  host.fetch.mockReset();
  vi.stubGlobal("fetch", host.fetch);
});

afterEach(() => vi.unstubAllGlobals());

it("keeps a locally ready Node's Installation reset page reachable without signing in", async () => {
  readyNative("ready");
  await expect(load()).resolves.toMatchObject({
    stage: "ready",
    bundled: true,
    hosts: expect.any(Array),
  });
  expect(host.invoke.mock.calls).toEqual([
    ["get_native_host_error"],
    ["get_setup_stage"],
  ]);
});

it("keeps Device completion reachable from the Installation reset page", async () => {
  readyNative("device");
  await expect(load()).resolves.toMatchObject({
    stage: "device",
    bundled: true,
    hosts: expect.any(Array),
  });
  expect(host.invoke.mock.calls).toEqual([
    ["get_native_host_error"],
    ["get_setup_stage"],
  ]);
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
  for (const host of hosts) expect(body).toContain(host.base);
  expect(body).toContain("Each owner requires its own code");
  expect(body).not.toContain("Type RESET to confirm");
  expect(body).not.toContain("Independent Create unavailable");
  expect(body).toContain("Administrator name");
  expect(body).toContain("Node name");
  expect(body).toContain("Recover exact operation across owners");
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

it.each([
  ["creating", "Creating your Node"],
  ["joining", "Joining the Installation"],
  ["unfinished", "Setup needs attention"],
] as const)(
  "shows a focused %s status after reload without manufacturing a reconnect intent",
  (state, title) => {
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
    expect(body).toContain(title);
    expect(body).toContain("Refresh status");
    expect(body).toContain("Resume setup");
    expect(body).toMatch(/<details>\s*<summary>Recovery options<\/summary>/);
    expect(body).not.toContain("Installation setup choice");
    expect(body).not.toContain(
      "Create a new Installation or join an existing one",
    );
    if (state === "unfinished") {
      expect(body).not.toContain("<progress");
      expect(body).toContain("Setup did not finish");
    } else {
      expect(body).toContain('aria-label="Setup in progress"');
      expect(body).toContain("Status updates automatically");
    }
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
              login_client: {
                client_id: "public-login",
                scopes: ["openid"],
              },
            },
          },
        ],
      },
    },
  });
  expect(body).toContain('href="/signin"');
  expect(body).toContain("Your Node is ready");
  expect(body).not.toContain('type="password"');
  expect(body).not.toContain("Resume setup");
  expect(body).not.toContain("Installation setup choice");
  expect(host.oidc).not.toHaveBeenCalled();
  expect(host.invoke).not.toHaveBeenCalled();
});

it.each([
  "awaiting_choice",
  "creating",
  "joining",
  "unfinished",
  "unavailable",
])(
  "direct web sign-in redirects %s to setup before login metadata or OIDC",
  async (state) => {
    host.native = false;
    host.fetch.mockResolvedValue(
      Response.json(
        {
          stage: "installation",
          state,
          ready: false,
          owners: ["idp", "management", "storage"],
        },
        { status: state === "unavailable" ? 503 : 200 },
      ),
    );
    const { load: signin } = await import("../../signin/+page");
    await expect(signin()).rejects.toMatchObject({
      status: 302,
      location: "/setup/installation",
    });
    expect(host.fetch).toHaveBeenCalledOnce();
    expect(host.clientId).not.toHaveBeenCalled();
    expect(host.oidc).not.toHaveBeenCalled();
    expect(host.invoke).not.toHaveBeenCalled();
  },
);

it.each([null, ""])(
  "direct web sign-in with missing endpoint %j opens configuration",
  async (base) => {
    host.native = false;
    host.idp = base;
    const { load: signin } = await import("../../signin/+page");
    await expect(signin()).rejects.toMatchObject({
      status: 302,
      location: "/config",
    });
    expect(host.fetch).not.toHaveBeenCalled();
    expect(host.clientId).not.toHaveBeenCalled();
    expect(host.oidc).not.toHaveBeenCalled();
  },
);

it("direct web sign-in awaits verified login metadata after readiness", async () => {
  host.native = false;
  readyNative("ready");
  const { load: signin } = await import("../../signin/+page");
  host.clientId.mockImplementationOnce(async () => {
    await Promise.resolve();
    expect(host.fetch).toHaveBeenCalledOnce();
    expect(host.oidc).not.toHaveBeenCalled();
    return "verified-client";
  });
  await expect(signin()).resolves.toBeUndefined();
  expect(host.clientId).toHaveBeenCalledOnce();
  expect(host.invoke).not.toHaveBeenCalled();
});

it.each(["network", "http", "malformed", "login"])(
  "direct web sign-in reports %s evidence failure as 503, not fresh setup",
  async (failureKind) => {
    host.native = false;
    readyNative("ready");
    if (failureKind === "network")
      host.fetch.mockRejectedValue(new TypeError("Failed to fetch"));
    if (failureKind === "http")
      host.fetch.mockResolvedValue(new Response(null, { status: 503 }));
    if (failureKind === "malformed")
      host.fetch.mockResolvedValue(Response.json({ ready: true }));
    if (failureKind === "login")
      host.clientId.mockRejectedValue(new Error("Login evidence unavailable"));
    const { load: signin } = await import("../../signin/+page");
    const failure = await signin().catch((cause: unknown) => cause);
    expect(isHttpError(failure, 503)).toBe(true);
    expect(failure).toMatchObject({
      body: { message: expect.stringContaining("Retry") },
    });
    if (failureKind !== "login") expect(host.clientId).not.toHaveBeenCalled();
    expect(host.oidc).not.toHaveBeenCalled();
    expect(host.invoke).not.toHaveBeenCalled();
  },
);

it.each(["installation", "device", "unavailable", "ready"])(
  "preserves native direct sign-in behavior for %s",
  async (stage) => {
    host.invoke.mockResolvedValue(stage);
    const { load: signin } = await import("../../signin/+page");
    if (stage === "ready") await expect(signin()).resolves.toBeUndefined();
    else
      await expect(signin()).rejects.toMatchObject({
        status: 302,
        location: "/setup",
      });
    expect(host.invoke.mock.calls).toEqual([["get_setup_stage"]]);
    expect(host.fetch).not.toHaveBeenCalled();
    expect(host.clientId).not.toHaveBeenCalled();
  },
);

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
