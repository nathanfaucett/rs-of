import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { OidcClient } from "./OidcClient.js";

const values = new Map<string, string>();
const codeClients = new Map<string, string>();
const issuer = "https://installation.test";
const metadata = {
  issuer,
  authorization_endpoint: "https://node.test/idp/oauth2/authorize",
  token_endpoint: "https://node.test/idp/oauth2/token",
  userinfo_endpoint: "https://node.test/idp/userinfo",
  registration_endpoint: "https://node.test/idp/oauth2/register",
};
const service = vi.fn(async (input: URL | RequestInfo, _init?: RequestInit) => {
  if (String(input).endsWith("/.well-known/openid-configuration"))
    return Response.json(metadata);
  if (String(input) === metadata.token_endpoint) {
    const body = new URLSearchParams(_init?.body as string);
    const code = body.get("code");
    if (code !== null && body.get("client_id") !== codeClients.get(code))
      return Response.json({ error: "invalid_client" }, { status: 400 });
    return Response.json({
      access_token: "access",
      refresh_token: "refresh",
      expires_in: 3600,
    });
  }
  if (String(input).includes("/oauth2/client/"))
    return Response.json({
      client_id: decodeURIComponent(
        String(input).split("/oauth2/client/")[1] ?? "",
      ),
      application: { uri: "app.test" },
    });
  if (String(input).endsWith("/userinfo"))
    return Response.json({ sub: "alice" });
  throw new Error("Unexpected request");
});
const config = () => ({
  authority: "https://node.test/idp",
  redirectUri: "https://app.test/callback",
  registration: {
    application: { uri: "app.test" },
    redirectUris: ["https://app.test/callback"],
    tokenEndpointAuthMethod: "none",
    clientId: "must-not-be-requested",
  },
});
const client = (overrides = {}) =>
  new OidcClient({
    projectOwned: true,
    fetch: service,
    disableNativeRequests: true,
    clientConfig: { ...config(), ...overrides },
  });
function callback(authorization: URL) {
  const url = new URL("https://app.test/callback");
  url.search = new URLSearchParams({
    code: "once",
    state: authorization.searchParams.get("state") ?? "",
    iss: issuer,
    client_id: "issued-by-idp",
  }).toString();
  return url;
}
beforeEach(() => {
  values.clear();
  codeClients.clear();
  codeClients.set("once", "issued-by-idp");
  service.mockClear();
  vi.stubGlobal("window", {
    location: { origin: "https://app.test" },
    localStorage: {
      get length() {
        return values.size;
      },
      key: (i: number) => [...values.keys()][i] ?? null,
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
      removeItem: (key: string) => values.delete(key),
    },
  });
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("onboards without registration POST and binds the issued ID to PKCE, reload, and refresh", async () => {
  const first = client();
  expect(first.registeredClientId).toBeUndefined();
  const authorization = await first.getAuthorizationUrl({
    extraParams: { client_id: "injected" },
  });
  expect(service).toHaveBeenCalledTimes(1);
  expect(authorization.searchParams.has("client_id")).toBe(false);
  expect(authorization.searchParams.get("application_id")).toBe("app.test");
  expect(
    JSON.parse(authorization.searchParams.get("registration") ?? "null"),
  ).not.toHaveProperty("client_id");
  const returning = client();
  await returning.handleSigninCallback(callback(authorization));
  expect(returning.registeredClientId).toBe("issued-by-idp");
  const body = new URLSearchParams(
    service.mock.calls.find(
      ([url]) => String(url) === metadata.token_endpoint,
    )?.[1]?.body as string,
  );
  expect(body.get("client_id")).toBe("issued-by-idp");
  expect(body.get("redirect_uri")).toBe("https://app.test/callback");
  expect(body.get("code")).toBe("once");
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(body.get("code_verifier") ?? ""),
  );
  const challenge = btoa(String.fromCharCode(...new Uint8Array(digest)))
    .replace(/=/g, "")
    .replace(/\+/g, "-")
    .replace(/\//g, "_");
  expect(challenge).toBe(authorization.searchParams.get("code_challenge"));
  await expect(
    returning.handleSigninCallback(callback(authorization)),
  ).rejects.toMatchObject({ code: "INVALID_STATE" });
  const reload = client();
  expect(reload.registeredClientId).toBe("issued-by-idp");
  expect(reload.getStoredTokenResponse()?.access_token).toBe("access");
  await reload.refreshToken();
  expect(
    new URLSearchParams(
      service.mock.calls
        .filter(([url]) => String(url) === metadata.token_endpoint)
        .at(-1)?.[1]?.body as string,
    ).get("client_id"),
  ).toBe("issued-by-idp");
  const next = await reload.getAuthorizationUrl();
  expect(next.searchParams.get("client_id")).toBe("issued-by-idp");
  reload.clearStoredTokenResponse();
  expect(client().registeredClientId).toBe("issued-by-idp");
  expect(
    client({ redirectUri: "https://app.test/other" }).registeredClientId,
  ).toBeUndefined();
  expect(
    client({
      registration: {
        ...config().registration,
        application: { uri: "other.test" },
      },
    }).registeredClientId,
  ).toBeUndefined();
  expect(
    client({ authority: "https://other.test/idp" }).registeredClientId,
  ).toBeUndefined();
});

test.each([
  "missing state",
  "unknown state",
  "missing id",
  "missing issuer",
  "issuer mismatch",
  "destination mismatch",
  "known id mismatch",
])("rejects %s before exchange or persisting an ID", async (failure) => {
  const overrides =
    failure === "known id mismatch" ? { clientId: "preprovisioned" } : {};
  const authorization = await client(overrides).getAuthorizationUrl();
  const url = callback(authorization);
  if (failure === "missing state") url.searchParams.delete("state");
  if (failure === "unknown state") url.searchParams.set("state", "unknown");
  if (failure === "missing id") url.searchParams.delete("client_id");
  if (failure === "missing issuer") url.searchParams.delete("iss");
  if (failure === "issuer mismatch")
    url.searchParams.set("iss", "https://foreign.test");
  if (failure === "destination mismatch") url.pathname = "/wrong";
  const returning = client(overrides);
  await expect(returning.handleSigninCallback(url)).rejects.toThrow();
  expect(returning.registeredClientId).toBe(overrides.clientId);
  expect([...values.keys()].some((key) => key.endsWith(":client"))).toBe(false);
  expect(
    service.mock.calls.every(([url]) =>
      String(url).endsWith("/.well-known/openid-configuration"),
    ),
  ).toBe(true);
});

test("does not persist an issued ID when token exchange fails", async () => {
  const url = callback(await client().getAuthorizationUrl());
  service.mockResolvedValueOnce(Response.json(metadata));
  service.mockResolvedValueOnce(
    Response.json({ error: "invalid_grant" }, { status: 400 }),
  );
  const returning = client();
  await expect(returning.handleSigninCallback(url)).rejects.toThrow();
  expect(client().registeredClientId).toBeUndefined();
  expect(client().getStoredTokenResponse()).toBeNull();
});

test("a late successful onboarding exchange cannot persist an ID after local termination", async () => {
  const returning = client();
  const url = callback(await returning.getAuthorizationUrl());
  let release!: (response: Response) => void;
  const held = new Promise<Response>((resolve) => {
    release = resolve;
  });
  service.mockReturnValueOnce(held);
  const pending = returning.handleSigninCallback(url);
  const rejected = expect(pending).rejects.toThrow(
    "Session ended during callback",
  );
  await vi.waitFor(() => expect(service).toHaveBeenCalledTimes(2));
  returning.clearStoredTokenResponse();
  release(Response.json({ access_token: "obsolete" }));
  await rejected;
  expect(client().registeredClientId).toBeUndefined();
  expect(client().getStoredTokenResponse()).toBeNull();
});

test("a successful HTTP response without an access token cannot persist an issued ID", async () => {
  const returning = client();
  const url = callback(await returning.getAuthorizationUrl());
  service.mockResolvedValueOnce(Response.json({}));
  await expect(returning.handleSigninCallback(url)).rejects.toThrow(
    "no access token",
  );
  expect(client().registeredClientId).toBeUndefined();
});

test("fixed IDs accept matching callback metadata and reject a mismatched issued ID after reload", async () => {
  const fixed = client({ clientId: "issued-by-idp" });
  await fixed.handleSigninCallback(callback(await fixed.getAuthorizationUrl()));
  expect(fixed.registeredClientId).toBe("issued-by-idp");
  const dynamic = client();
  await dynamic.handleSigninCallback(
    callback(await dynamic.getAuthorizationUrl()),
  );
  const reload = client();
  const url = callback(await reload.getAuthorizationUrl());
  url.searchParams.set("client_id", "another-client");
  await expect(reload.handleSigninCallback(url)).rejects.toThrow();
  expect(client().registeredClientId).toBe("issued-by-idp");
});

test("a re-issued client ID replaces a deleted one and binds the exchange to it", async () => {
  const first = client();
  await first.handleSigninCallback(callback(await first.getAuthorizationUrl()));
  expect(client().registeredClientId).toBe("issued-by-idp");
  const reissued = client();
  const url = callback(await reissued.getAuthorizationUrl());
  url.searchParams.set("client_id", "reissued-by-idp");
  codeClients.set("once", "reissued-by-idp");
  await reissued.handleSigninCallback(url);
  expect(reissued.registeredClientId).toBe("reissued-by-idp");
  expect(client().registeredClientId).toBe("reissued-by-idp");
  const body = new URLSearchParams(
    service.mock.calls
      .filter(([url]) => String(url) === metadata.token_endpoint)
      .at(-1)?.[1]?.body as string,
  );
  expect(body.get("client_id")).toBe("reissued-by-idp");
});

test.each([
  [undefined, "client"],
  [undefined, "application"],
  [undefined, "missing application"],
  [undefined, "http"],
  [undefined, "network"],
  [undefined, "malformed"],
  ["issued-by-idp", "client"],
  ["issued-by-idp", "application"],
  ["issued-by-idp", "http"],
])(
  "rejects invalid server-owned metadata (%s, %s) before retaining credentials",
  async (clientId, failure) => {
    const returning = client({ clientId });
    const url = callback(await returning.getAuthorizationUrl());
    service.mockResolvedValueOnce(
      Response.json({
        access_token: "new-access",
        refresh_token: "new-refresh",
      }),
    );
    if (failure === "network")
      service.mockRejectedValueOnce(new Error("unavailable"));
    else if (failure === "http")
      service.mockResolvedValueOnce(new Response(null, { status: 403 }));
    else if (failure === "malformed")
      service.mockResolvedValueOnce(new Response("not-json"));
    else
      service.mockResolvedValueOnce(
        Response.json({
          client_id: failure === "client" ? "other-client" : "issued-by-idp",
          application:
            failure === "missing application"
              ? undefined
              : { uri: failure === "application" ? "other.test" : "app.test" },
        }),
      );
    await expect(returning.handleSigninCallback(url)).rejects.toThrow();
    const [endpoint, init] = service.mock.calls.at(-1) ?? [];
    expect(String(endpoint)).toBe(
      "https://node.test/idp/oauth2/client/issued-by-idp",
    );
    expect(new Headers(init?.headers).get("Authorization")).toBe(
      "Bearer new-access",
    );
    expect(init?.redirect).toBe("error");
    expect(returning.registeredClientId).toBe(clientId);
    expect(returning.getStoredTokenResponse()).toBeNull();
    expect(
      [...values.keys()].some(
        (key) => key.endsWith(":client") || key.endsWith(":response"),
      ),
    ).toBe(false);
  },
);

test("metadata is verified before persistence and a late metadata result cannot revive onboarding", async () => {
  const returning = client();
  const url = callback(await returning.getAuthorizationUrl());
  let release!: (response: Response) => void;
  const held = new Promise<Response>((resolve) => {
    release = resolve;
  });
  service.mockResolvedValueOnce(Response.json({ access_token: "new-access" }));
  service.mockReturnValueOnce(held);
  const pending = returning.handleSigninCallback(url);
  const rejected = expect(pending).rejects.toThrow(
    "Session ended during callback",
  );
  await vi.waitFor(() => expect(service).toHaveBeenCalledTimes(3));
  expect(returning.registeredClientId).toBeUndefined();
  expect(returning.getStoredTokenResponse()).toBeNull();
  returning.clearStoredTokenResponse();
  release(
    Response.json({
      client_id: "issued-by-idp",
      application: { uri: "app.test" },
    }),
  );
  await rejected;
  expect(client().registeredClientId).toBeUndefined();
  expect(client().getStoredTokenResponse()).toBeNull();
});

test("refresh validates metadata before storing replacement credentials", async () => {
  const returning = client();
  await returning.handleSigninCallback(
    callback(await returning.getAuthorizationUrl()),
  );
  const stored = returning.getStoredTokenResponse();
  service.mockResolvedValueOnce(Response.json({ access_token: "refreshed" }));
  service.mockResolvedValueOnce(
    Response.json({
      client_id: "issued-by-idp",
      application: { uri: "other.test" },
    }),
  );
  await expect(returning.refreshToken()).rejects.toMatchObject({
    code: "INVALID_CLIENT_METADATA",
  });
  expect(returning.getStoredTokenResponse()).toEqual(stored);
  expect(
    new Headers(service.mock.calls.at(-1)?.[1]?.headers).get("Authorization"),
  ).toBe("Bearer refreshed");
});

test("external OIDC still dynamically registers before authorization", async () => {
  const external = new OidcClient({
    clientConfig: config(),
    storage: "memory",
    fetch: service,
  });
  service.mockResolvedValueOnce(Response.json(metadata));
  service.mockResolvedValueOnce(
    Response.json({ client_id: "external-issued" }),
  );
  const url = await external.getAuthorizationUrl();
  expect(url.searchParams.get("client_id")).toBe("external-issued");
  expect(external.registeredClientId).toBe("external-issued");
  expect(String(service.mock.calls[1][0])).toBe(metadata.registration_endpoint);
  expect(service.mock.calls[1][1]?.method).toBe("POST");
});
