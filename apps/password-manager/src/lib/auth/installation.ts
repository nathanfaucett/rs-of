import type { OidcAuthConfig } from "./oidc";

const setupRequired =
  "Finish Node setup before signing in to sync. You can still use local vaults.";

export class NodeSetupRequiredError extends Error {
  override name = "NodeSetupRequiredError";
  readonly setupUrl: string;

  constructor(setupUrl: string) {
    super(setupRequired);
    this.setupUrl = setupUrl;
  }
}

function serviceBase(value: unknown): string {
  if (typeof value !== "string") throw new Error("Missing service URL");
  const url = new URL(value);
  const loopback = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  if (
    (url.protocol !== "https:" && !(url.protocol === "http:" && loopback)) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  )
    throw new Error("Invalid service URL");
  return url.href.replace(/\/+$/, "");
}

async function selectedInstallation(
  origin: string,
  setupUrl: string
): Promise<{ idp: string; storage: string }> {
  try {
    const node = serviceBase(origin);
    if (node !== new URL(node).origin) throw new Error("Expected a Node origin");
    const response = await fetch(`${node}/setup/node`, {
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
      signal: AbortSignal.timeout(5000),
    });
    if (response.ok) {
      const state = await response.json();
      if (state?.state === "ready") {
        return { idp: serviceBase(state.idp), storage: serviceBase(state.storage) };
      }
    }
  } catch {
    // An unreachable or invalid Node descriptor cannot authorize sign-in.
  }
  throw new NodeSetupRequiredError(setupUrl);
}

export async function authConfig(
  env: Record<string, string | undefined>,
  redirectUri: string
): Promise<OidcAuthConfig> {
  const {
    PUBLIC_UNIFIED_ORIGIN = "https://unified.localhost:1355",
    PUBLIC_NODE_SETUP_URL = "https://of.localhost:1355/setup",
    PUBLIC_IDP_BASE_URL,
    PUBLIC_STORAGE_URL,
    PUBLIC_STORAGE_BASE_URL,
    PUBLIC_OIDC_CLIENT_ID: clientId,
    PUBLIC_OIDC_APPLICATION_URI: applicationUri,
    PUBLIC_OIDC_SCOPES: scopes = "openid profile email storage",
    PUBLIC_OIDC_REDIRECT_URI,
  } = env;
  if (clientId) {
    const storage = PUBLIC_STORAGE_URL ?? PUBLIC_STORAGE_BASE_URL;
    if (!PUBLIC_IDP_BASE_URL || !storage || !applicationUri) {
      throw new Error(
        "Configure the IdP, Storage, and Application URI for the fixed OAuth Client."
      );
    }
    return {
      idp: serviceBase(PUBLIC_IDP_BASE_URL),
      storage: serviceBase(storage),
      clientId,
      redirectUri: PUBLIC_OIDC_REDIRECT_URI ?? redirectUri,
      scopes,
      applicationUri,
    };
  }
  return {
    ...(await selectedInstallation(PUBLIC_UNIFIED_ORIGIN, PUBLIC_NODE_SETUP_URL)),
    redirectUri: PUBLIC_OIDC_REDIRECT_URI ?? redirectUri,
    scopes,
    applicationUri: applicationUri ?? new URL(redirectUri).origin,
  };
}
