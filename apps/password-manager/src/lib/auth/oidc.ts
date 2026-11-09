import { OidcClient, type OidcClientOptions, type OidcTokenResponse, type OidcUserInfo } from "@ofsdk/oidc-client";
import { env } from "$env/dynamic/public";

export interface OidcAuthConfig {
  authority: string;
  clientId: string;
  redirectUri: string;
  scopes?: string;
}

export class OidcAuth {
  private client: OidcClient | null = null;
  private config: OidcAuthConfig | null = null;
  private refreshTimeout: ReturnType<typeof setTimeout> | null = null;

  async initialize(config: OidcAuthConfig): Promise<void> {
    this.config = config;

    const clientConfig: OidcClientOptions = {
      projectOwned: true,
      clientConfig: {
        authority: config.authority,
        clientId: config.clientId,
        redirectUri: config.redirectUri,
        responseType: "code",
        registration: {
          clientId: config.clientId,
          clientName: "Password Manager",
          scope: config.scopes ?? "openid profile email storage:read storage:write",
          redirectUris: [config.redirectUri],
          postLogoutRedirectUris: [config.redirectUri.replace("/callback", "")],
          grantTypes: ["authorization_code", "refresh_token"],
          responseTypes: ["code"],
          tokenEndpointAuthMethod: "none",
          clientType: "public",
          profile: "web_application",
        },
      },
      storage: "memory",
      disableNativeRequests: true,
    };

    this.client = new OidcClient(clientConfig);
    await this.client.getOidcConfiguration();
  }

  async login(): Promise<void> {
    if (!this.client) throw new Error("OidcAuth not initialized");
    await this.client.signin();
  }

  async handleCallback(): Promise<OidcTokenResponse | null> {
    if (!this.client) throw new Error("OidcAuth not initialized");

    const result = await this.client.handleSigninCallback();
    if (result) {
      this.scheduleTokenRefresh();
    }
    return result;
  }

  async getAccessToken(): Promise<string> {
    if (!this.client) throw new Error("OidcAuth not initialized");

    const tokenResponse = this.client.getStoredTokenResponse();
    if (!tokenResponse?.access_token) {
      throw new Error("No access token available");
    }

    const expiresAt = tokenResponse.access_token_expires_at;
    if (expiresAt && Date.now() >= expiresAt - 60_000) {
      await this.refreshToken();
    }

    return tokenResponse.access_token;
  }

  private async refreshToken(): Promise<void> {
    if (!this.client) throw new Error("OidcAuth not initialized");

    try {
      await this.client.refreshToken();
      this.scheduleTokenRefresh();
    } catch (error) {
      console.error("Token refresh failed:", error);
      this.clearTokens();
      throw error;
    }
  }

  private scheduleTokenRefresh(): void {
    if (this.refreshTimeout) {
      clearTimeout(this.refreshTimeout);
    }

    const tokenResponse = this.client?.getStoredTokenResponse();
    if (!tokenResponse?.access_token_expires_at) return;

    const expiresAt = tokenResponse.access_token_expires_at;
    const delay = Math.max(0, expiresAt - Date.now() - 60_000);

    this.refreshTimeout = setTimeout(() => {
      void this.refreshToken();
    }, delay);
  }

  async logout(): Promise<void> {
    if (this.client) {
      await this.client.signout();
      this.client.clearStoredTokenResponse();
    }
    if (this.refreshTimeout) {
      clearTimeout(this.refreshTimeout);
      this.refreshTimeout = null;
    }
  }

  async getUserInfo(): Promise<OidcUserInfo | null> {
    if (!this.client) return null;
    return this.client.getUserInfo();
  }

  isAuthenticated(): boolean {
    if (!this.client) return false;
    const tokenResponse = this.client.getStoredTokenResponse();
    return !!tokenResponse?.access_token;
  }

  private clearTokens(): void {
    if (this.client) {
      this.client.clearStoredTokenResponse();
    }
    if (this.refreshTimeout) {
      clearTimeout(this.refreshTimeout);
      this.refreshTimeout = null;
    }
  }
}

export const oidcAuth = new OidcAuth();
