import type { OidcClient, OidcUserInfo } from "@ofsdk/oidc-client";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { env } from "$env/dynamic/public";
import { authConfig, NodeSetupRequiredError } from "./installation";
import { createOidcClient, type OidcAuthConfig } from "./oidc";

let client: OidcClient | undefined;
let startup: Promise<void> | undefined;
let verifiedToken: string | undefined;
type SessionListener = (ended: boolean, previous: typeof session) => void;
const sessionListeners = new Set<SessionListener>();

function notifySession(ended: boolean, previous: typeof session) {
  for (const listener of sessionListeners) listener(ended, previous);
}

function endSession() {
  const previous = session;
  session = null;
  verifiedToken = undefined;
  notifySession(true, previous);
}

function watchSession(listener: SessionListener) {
  sessionListeners.add(listener);
  return () => {
    sessionListeners.delete(listener);
  };
}

function checkStoredSession(event: StorageEvent) {
  const expected = session;
  const oidc = client;
  if (!oidc?.isStoredTokenEvent(event)) return;
  // A later sign-in can replace storage before this removal event is delivered.
  if (event.newValue === null) {
    client = undefined;
    if (expected) endSession();
    return;
  }
  if (!expected) return;
  const current = oidc.getStoredTokenResponse()?.access_token;
  if (current === verifiedToken) return;
  // A token replacement may be a renewal or a different User. Stop transport before checking.
  notifySession(false, expected);
  if (!current) {
    endSession();
    return;
  }
  void getAccessToken(expected).catch(() => {
    if (
      session === expected &&
      current === oidc.getStoredTokenResponse()?.access_token &&
      current !== verifiedToken
    )
      endSession();
  });
}
let session = $state.raw<{
  user: OidcUserInfo;
  userEmail: string | undefined;
  issuer: string;
  applicationUri: string;
  idp: string;
  storage: string;
  clientId: string;
} | null>(null);
let error = $state<string | null>(null);
let setupUrl = $state<string | null>(null);

function config() {
  return authConfig(env, new URL(resolve("/callback"), window.location.origin).href);
}

function showError(cause: unknown, fallback: string) {
  setupUrl = cause instanceof NodeSetupRequiredError ? cause.setupUrl : null;
  error = cause instanceof Error ? cause.message : fallback;
}

async function restore(): Promise<void> {
  try {
    const selected = await config();
    const oidc = createOidcClient(selected);
    client = oidc;
    if (window.location.pathname === new URL(selected.redirectUri).pathname) {
      const params = new URLSearchParams(window.location.search);
      if (params.has("error")) throw new Error(params.get("error") ?? "Authentication failed");
      if (params.has("code")) {
        await oidc.handleSigninCallback();
        window.history.replaceState(window.history.state, "", window.location.pathname);
      }
    }
    if (oidc.getStoredTokenResponse()) await establishSession(oidc, selected);
  } catch (cause) {
    showError(cause, "Authentication failed");
  }
}

async function establishSession(
  oidc: OidcClient,
  { idp, storage, applicationUri }: OidcAuthConfig
) {
  const { issuer, userInfo, accessToken } = await oidc.getVerifiedIdentity();
  if (client !== oidc) return;
  verifiedToken = accessToken;
  const clientId = oidc.registeredClientId;
  if (!clientId) throw new Error("No verified OAuth Client ID");
  const { email } = userInfo;
  const userEmail = typeof email === "string" ? email : undefined;
  session = { user: userInfo, userEmail, issuer, applicationUri, idp, storage, clientId };
}

function initialize(): Promise<void> {
  startup ??= restore();
  return startup;
}

async function login(): Promise<void> {
  await initialize();
  error = null;
  setupUrl = null;
  try {
    if (session) {
      await goto(resolve("/"));
      return;
    }
    client = createOidcClient(await config());
    await client.signin();
  } catch (cause) {
    showError(cause, "Login failed");
  }
}

async function logout(): Promise<void> {
  const oidc = client;
  client = undefined;
  try {
    endSession();
  } finally {
    oidc?.clearStoredTokenResponse();
  }
  await goto(resolve("/"));
}

function hasVerifiedToken(expected: NonNullable<typeof session>): boolean {
  return (
    session === expected &&
    verifiedToken !== undefined &&
    client?.getStoredTokenResponse()?.access_token === verifiedToken
  );
}

async function getAccessToken(expected: NonNullable<typeof session>): Promise<string> {
  const oidc = client;
  if (!oidc || session !== expected) throw new Error("Sign in to sync first");
  // Another tab can replace a stored token without changing this tab's session.
  const identity = await oidc.getVerifiedIdentity();
  if (client !== oidc || session !== expected) throw new Error("Sign-in session ended");
  if (
    identity.issuer !== expected.issuer ||
    identity.subject !== expected.user.sub ||
    oidc.registeredClientId !== expected.clientId
  ) {
    endSession();
    throw new Error("The signed-in account changed. Reload and confirm the destination again.");
  }
  verifiedToken = identity.accessToken;
  return identity.accessToken;
}

export const authState = {
  get session() {
    return session;
  },
  get isAuthenticated() {
    return session !== null;
  },
  get error() {
    return error;
  },
  get setupUrl() {
    return setupUrl;
  },
  initialize,
  login,
  logout,
  getAccessToken,
  hasVerifiedToken,
  checkStoredSession,
  watchSession,
};
