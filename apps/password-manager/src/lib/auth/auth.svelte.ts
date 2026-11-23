import { goto } from "$app/navigation";
import { env } from "$env/dynamic/public";
import { type OidcAuthConfig, oidcAuth } from "./oidc";

interface UserInfo {
  sub: string;
  email?: string | undefined;
  name?: string | undefined;
  preferred_username?: string | undefined;
}

const DEFAULT_IDP_BASE = "https://idp.localhost:1355";
const DEFAULT_CLIENT_ID = "password-manager";
const DEFAULT_REDIRECT_URI = "https://password-manager.localhost:1355/callback";
const DEFAULT_SCOPES = "openid profile email storage:read storage:write";

let user = $state<UserInfo | null>(null);
let isLoading = $state(true);
let error = $state<string | null>(null);

function getIdpBaseUrl(): string {
  const { PUBLIC_IDP_BASE_URL: baseUrl } = env;
  return baseUrl ?? DEFAULT_IDP_BASE;
}

function getClientId(): string {
  const { PUBLIC_OIDC_CLIENT_ID: clientId } = env;
  return clientId ?? DEFAULT_CLIENT_ID;
}

function getRedirectUri(): string {
  const { PUBLIC_OIDC_REDIRECT_URI: redirectUri } = env;
  return redirectUri ?? DEFAULT_REDIRECT_URI;
}

async function initialize(): Promise<void> {
  isLoading = true;
  error = null;

  try {
    const {
      PUBLIC_OIDC_APPLICATION_URI: applicationUri,
      PUBLIC_OIDC_SCOPES: scopes = DEFAULT_SCOPES,
    } = env;
    const config: OidcAuthConfig = {
      authority: getIdpBaseUrl(),
      ...(applicationUri === undefined ? {} : { applicationUri }),
      clientId: getClientId(),
      redirectUri: getRedirectUri(),
      scopes,
    };

    await oidcAuth.initialize(config);

    const urlParams = new URLSearchParams(window.location.search);
    const code = urlParams.get("code");
    const errorParam = urlParams.get("error");

    if (errorParam) {
      error = errorParam;
      isLoading = false;
      return;
    }

    if (code) {
      await oidcAuth.handleCallback();
      window.history.replaceState({}, document.title, window.location.pathname);
    }

    if (oidcAuth.isAuthenticated()) {
      const userInfo = await oidcAuth.getUserInfo();
      if (userInfo) {
        const { email, name, preferred_username } = userInfo as Record<string, unknown>;
        user = {
          sub: userInfo.sub,
          email: email as string | undefined,
          name: name as string | undefined,
          preferred_username: preferred_username as string | undefined,
        };
      }
    }
  } catch (e) {
    error = e instanceof Error ? e.message : "Authentication failed";
  } finally {
    isLoading = false;
  }
}

async function login(): Promise<void> {
  error = null;
  try {
    await oidcAuth.login();
  } catch (e) {
    error = e instanceof Error ? e.message : "Login failed";
  }
}

async function logout(): Promise<void> {
  await oidcAuth.logout();
  user = null;
  await goto("/");
}

async function getAccessToken(): Promise<string> {
  return oidcAuth.getAccessToken();
}

function getIsAuthenticated(): boolean {
  return oidcAuth.isAuthenticated();
}

export const authState = {
  get user() {
    return user;
  },
  get isAuthenticated() {
    return getIsAuthenticated();
  },
  get isLoading() {
    return isLoading;
  },
  get error() {
    return error;
  },
  initialize,
  login,
  logout,
  getAccessToken,
};
