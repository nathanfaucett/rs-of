import type { OidcClient, OidcUserInfo } from "@ofsdk/oidc-client";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { env } from "$env/dynamic/public";
import { authConfig, NodeSetupRequiredError } from "./installation";
import { createOidcClient, type OidcAuthConfig } from "./oidc";

let client: OidcClient | undefined;
let logoutPending: OidcClient | undefined;
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
  const stored = oidc.getStoredTokenResponse();
  const current = stored?.access_token;
  if (current === verifiedToken && stored?.state === expected.loginState) return;
  // A token replacement may be a renewal or a new login. Stop transport before checking.
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
  loginState: string;
} | null>(null);
let error = $state<string | null>(null);
let logoutError = $state<string | null>(null);
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
  const { issuer, userInfo, accessToken, loginState } = await oidc.getVerifiedIdentity();
  if (client !== oidc) return;
  verifiedToken = accessToken;
  const clientId = oidc.registeredClientId;
  if (!clientId || !loginState)
    throw new Error("No verified login session. Sign in again to use Storage.");
  const { email } = userInfo;
  const userEmail = typeof email === "string" ? email : undefined;
  session = {
    user: userInfo,
    userEmail,
    issuer,
    applicationUri,
    idp,
    storage,
    clientId,
    loginState,
  };
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
    if (logoutPending) throw new Error(logoutError ?? "Finish signing out first.");
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
  const oidc = client ?? logoutPending;
  client = undefined;
  endSession();
  try {
    oidc?.clearStoredTokenResponse();
  } catch {
    logoutPending = oidc;
    logoutError =
      "Sign-out is incomplete. This tab stopped sync, but the browser could not clear saved credentials. Reloading may sign you back in. Fix browser storage and retry sign-out.";
    return;
  }
  logoutPending = undefined;
  logoutError = null;
  await goto(resolve("/"));
}

function hasVerifiedToken(expected: NonNullable<typeof session>): boolean {
  const stored = client?.getStoredTokenResponse();
  return (
    session === expected &&
    verifiedToken !== undefined &&
    stored?.access_token === verifiedToken &&
    stored.state === expected.loginState
  );
}

async function getAccessToken(
  expected: NonNullable<typeof session>,
  signal?: AbortSignal
): Promise<string> {
  signal?.throwIfAborted();
  const oidc = client;
  if (!oidc || session !== expected) throw new Error("Sign in to sync first");
  // Another tab can replace a stored token without changing this tab's session.
  const identity = await oidc.getVerifiedIdentity(signal);
  signal?.throwIfAborted();
  if (client !== oidc || session !== expected) throw new Error("Sign-in session ended");
  if (
    identity.issuer !== expected.issuer ||
    identity.subject !== expected.user.sub ||
    identity.loginState !== expected.loginState ||
    oidc.registeredClientId !== expected.clientId
  ) {
    endSession();
    throw new Error("The sign-in session changed. Reload and confirm the destination again.");
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
  get logoutError() {
    return logoutError;
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
