import { goto } from "$app/navigation";
import { oidcAuth, type OidcAuthConfig } from "./oidc";
import { env } from "$env/dynamic/public";

interface UserInfo {
  sub: string;
  email?: string;
  name?: string;
  preferred_username?: string;
}

const DEFAULT_IDP_BASE = "https://idp.localhost:1355";
const DEFAULT_CLIENT_ID = "password-manager";
const DEFAULT_REDIRECT_URI = "https://password-manager.localhost:1355/callback";
const DEFAULT_SCOPES = "openid profile email storage:read storage:write";

let user = $state<UserInfo | null>(null);
let isLoading = $state(true);
let error = $state<string | null>(null);

function getIdpBaseUrl(): string {
  return env.PUBLIC_IDP_BASE_URL ?? DEFAULT_IDP_BASE;
}

function getClientId(): string {
  return env.PUBLIC_OIDC_CLIENT_ID ?? DEFAULT_CLIENT_ID;
}

function getRedirectUri(): string {
  return env.PUBLIC_OIDC_REDIRECT_URI ?? DEFAULT_REDIRECT_URI;
}

async function initialize(): Promise<void> {
  isLoading = true;
  error = null;

  try {
    const config: OidcAuthConfig = {
      authority: getIdpBaseUrl(),
      clientId: getClientId(),
      redirectUri: getRedirectUri(),
      scopes: DEFAULT_SCOPES,
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
        user = {
          sub: userInfo.sub,
          email: (userInfo as Record<string, unknown>).email as string | undefined,
          name: (userInfo as Record<string, unknown>).name as string | undefined,
          preferred_username: (userInfo as Record<string, unknown>).preferred_username as string | undefined,
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
