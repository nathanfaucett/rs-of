import {
  AuthorizationServerMetadataToJSON,
  Configuration,
  DefaultApi,
  TokenGrantTypeEnum,
  TokenResponseToJSON,
  UserInfoToJSON,
} from "@aicacia/idp-client";
import {
  createNativeFetch,
  generateState,
  isNativeProtocol,
  type NativeFetch,
  openUrl,
} from "@aicacia/native-fetch";
import { EventEmitter } from "eventemitter3";
import type { OidcClientConfig } from "./OidcClientConfig.js";
import { OidcClientError } from "./OidcClientError.js";
import type { OidcClientMetadata } from "./OidcClientMetadata.js";
import type { OidcClientRegistrationResponse } from "./OidcClientRegistrationResponse.js";
import type { OidcConfiguration } from "./OidcConfiguration.js";
import {
  createFetchWithTimeout,
  type Fetch,
} from "./util/createFetchWithTimeout.js";
import { generatePkcePair } from "./util/generatePkcePair.js";
import { snakeCase, snakeCaseKeys } from "./util/snakeCaseKeys.js";

export type OidcClientEvents = {
  registered: (response: OidcClientRegistrationResponse) => void;
  error: (error: unknown) => void;
};

export type OidcClientOptions = {
  projectOwned?: boolean;
  clientConfig: OidcClientConfig;
  disableNativeRequests?: boolean;
  storage?: "local" | "memory";
  fetch?: Fetch;
};

export type AuthorizationUrlOptions = {
  state?: string;
  extraParams?: Record<string, string | number | boolean>;
};

export type RegistrationOptions = {
  redirectUri?: string;
  registration?: OidcClientMetadata;
  popup?: boolean;
  windowFeatures?: string;
};

const PKCE_STORAGE_PREFIX = "oidc.pkce";
const TOKEN_STORAGE_PREFIX = "oidc.token";

const OPTIONAL_AUTHORIZATION_PARAMS = [
  "prompt",
  "display",
  "maxAge",
  "uiLocales",
  "acrValues",
  "resource",
  "responseMode",
] as const;

export type SigninOptions = AuthorizationUrlOptions & {
  popup?: boolean;
  extraParams?: Record<string, string | number | boolean>;
  windowFeatures?: string;
};

export type SignoutOptions = {
  redirectUri?: string;
  extraParams?: Record<string, string | number | boolean>;
  popup?: boolean;
  windowFeatures?: string;
};

export type OidcUserInfo = {
  sub: string;
  [key: string]: unknown;
};

export type OidcTokenResponse = {
  access_token?: string;
  id_token?: string;
  refresh_token?: string;
  token_type?: string;
  expires_in?: number | string;
  access_token_expires_at?: number;
  refresh_token_expires_in?: number | string;
  scope?: string;
  state?: string;
  [key: string]: unknown;
};

export class OidcClient<
  UserInfo extends OidcUserInfo = OidcUserInfo,
> extends EventEmitter<OidcClientEvents> {
  private readonly config: OidcClientConfig;
  private readonly projectOwned: boolean;
  private readonly disableNativeRequests: boolean;
  private readonly nativeFetch: NativeFetch;
  private readonly fetch: Fetch;
  private readonly memoryOnly: boolean;
  private tokenResponse: OidcTokenResponse | null = null;
  private issuedClient: { clientId: string; issuer: string } | undefined;
  private sessionGeneration = 0;
  private identityContext = 0;
  private refreshPending: {
    generation: number;
    promise: Promise<OidcTokenResponse>;
  } | null = null;

  private oidcConfigPromise: Promise<OidcConfiguration> | null = null;
  private readonly pkceVerifiersByState = new Map<string, string>();
  private readonly pkceStoragePrefix: string;
  private readonly tokenStoragePrefix: string;

  constructor(options: OidcClientOptions) {
    super();
    this.config = OidcClient.validateConfig(options.clientConfig);
    this.projectOwned = options.projectOwned ?? false;
    const namespace = encodeURIComponent(
      JSON.stringify([
        this.config.authority,
        this.config.clientId ?? this.config.registration,
        this.getRedirectUri(),
        this.config.registration,
      ]),
    );
    this.pkceStoragePrefix = `${PKCE_STORAGE_PREFIX}:${namespace}`;
    this.tokenStoragePrefix = `${TOKEN_STORAGE_PREFIX}:${namespace}`;
    this.memoryOnly = options.storage === "memory";

    this.disableNativeRequests = options.disableNativeRequests ?? false;
    this.nativeFetch = createNativeFetch(this.config.authority);
    this.fetch = this.config.requestTimeoutInSeconds
      ? createFetchWithTimeout(
          this.config.requestTimeoutInSeconds * 1000,
          options.fetch,
        )
      : (options.fetch ?? fetch);
  }

  private getIssuedClient(): { clientId: string; issuer: string } | undefined {
    if (this.memoryOnly) return this.issuedClient;
    const value = this.getStorage()?.getItem(
      `${this.tokenStoragePrefix}:client`,
    );
    if (!value) return undefined;
    try {
      const client = JSON.parse(value);
      if (
        typeof client.clientId === "string" &&
        client.clientId.trim() &&
        typeof client.issuer === "string"
      )
        return client;
    } catch {}
    return undefined;
  }

  get registeredClientId(): string | undefined {
    return this.config.clientId ?? this.getIssuedClient()?.clientId;
  }

  private rememberIssuedClient(clientId: string, issuer: string): void {
    const client = { clientId, issuer };
    if (this.memoryOnly) this.issuedClient = client;
    else
      this.getStorage()?.setItem(
        `${this.tokenStoragePrefix}:client`,
        JSON.stringify(client),
      );
  }

  private ownerApi(headers: Record<string, string> = {}): DefaultApi {
    return new DefaultApi(
      new Configuration({
        basePath: this.config.authority,
        fetchApi: this.fetch,
        headers,
        credentials: this.config.fetchRequestCredentials ?? "same-origin",
      }),
    );
  }

  private requireOwnerEndpoint(endpoint: URL | string, path: string): void {
    if (new URL(endpoint).href !== new URL(this.config.authority + path).href)
      throw new Error("Discovery does not match the configured owner endpoint");
  }

  private getStorage(): Storage | null {
    if (this.memoryOnly || typeof window === "undefined") {
      return null;
    }
    try {
      return window.localStorage;
    } catch {
      return null;
    }
  }

  private getPkceStorageKey(state: string): string {
    return `${this.pkceStoragePrefix}:state:${state}`;
  }

  private getTokenStorageKey(): string {
    return `${this.tokenStoragePrefix}:response`;
  }

  private rememberTokenResponse(tokenResponse: OidcTokenResponse): void {
    this.sessionGeneration += 1;
    if (this.memoryOnly) {
      this.tokenResponse = tokenResponse;
      return;
    }
    const storage = this.getStorage();
    if (!storage) {
      return;
    }
    storage.setItem(this.getTokenStorageKey(), JSON.stringify(tokenResponse));
  }

  getStoredTokenResponse(): OidcTokenResponse | null {
    if (this.memoryOnly) return this.tokenResponse;
    const storage = this.getStorage();
    if (!storage) {
      return null;
    }
    const value = storage.getItem(this.getTokenStorageKey());
    if (!value) {
      return null;
    }
    try {
      return JSON.parse(value) as unknown as OidcTokenResponse;
    } catch {
      return null;
    }
  }

  setToken(tokenResponse: OidcTokenResponse) {
    this.identityContext += 1;
    this.rememberTokenResponse(this.normalizeTokenResponse(tokenResponse));
  }

  clearStoredTokenResponse(): void {
    this.identityContext += 1;
    this.sessionGeneration += 1;
    this.tokenResponse = null;
    this.pkceVerifiersByState.clear();
    const storage = this.getStorage();
    if (!storage) return;
    storage.removeItem(this.getTokenStorageKey());
    for (let index = storage.length - 1; index >= 0; index--) {
      const key = storage.key(index);
      if (key?.startsWith(`${this.pkceStoragePrefix}:`))
        storage.removeItem(key);
    }
  }

  private rememberPkceVerifier(state: string, codeVerifier: string): void {
    this.pkceVerifiersByState.set(state, codeVerifier);

    const storage = this.getStorage();
    if (!storage) {
      return;
    }
    storage.setItem(this.getPkceStorageKey(state), codeVerifier);
  }

  private consumePkceVerifier(state?: string | null): string | undefined {
    const storage = this.getStorage();
    if (state) {
      const memoryValue = this.pkceVerifiersByState.get(state);
      this.pkceVerifiersByState.delete(state);
      const storageKey = this.getPkceStorageKey(state);
      const storageValue = storage?.getItem(storageKey) ?? null;
      storage?.removeItem(storageKey);
      return memoryValue ?? storageValue ?? undefined;
    }

    return undefined;
  }

  private readCallbackParam(
    queryParams: URLSearchParams,
    hashParams: URLSearchParams,
    name: string,
  ): string | null {
    return queryParams.get(name) ?? hashParams.get(name);
  }

  private getHashParams(url: URL): URLSearchParams {
    let hash = "";

    try {
      hash = url.hash;
    } catch {
      if (typeof window !== "undefined") {
        hash = new URL(window.location.href).hash;
      }
    }

    return new URLSearchParams(hash.startsWith("#") ? hash.slice(1) : hash);
  }

  private normalizeTokenResponse(input: OidcTokenResponse): OidcTokenResponse {
    const expiresInRaw = input.expires_in;
    const expiresIn =
      typeof expiresInRaw === "number"
        ? expiresInRaw
        : typeof expiresInRaw === "string"
          ? Number.parseInt(expiresInRaw, 10)
          : undefined;
    const refreshTokenExpiresInRaw = input.refresh_token_expires_in;
    const refreshTokenExpiresIn =
      typeof refreshTokenExpiresInRaw === "number"
        ? refreshTokenExpiresInRaw
        : typeof refreshTokenExpiresInRaw === "string"
          ? Number.parseInt(refreshTokenExpiresInRaw, 10)
          : undefined;

    return {
      ...input,
      access_token:
        typeof input.access_token === "string" ? input.access_token : undefined,
      id_token: typeof input.id_token === "string" ? input.id_token : undefined,
      refresh_token:
        typeof input.refresh_token === "string"
          ? input.refresh_token
          : undefined,
      token_type:
        typeof input.token_type === "string" ? input.token_type : undefined,
      expires_in: Number.isNaN(expiresIn ?? Number.NaN) ? undefined : expiresIn,
      access_token_expires_at:
        typeof input.access_token_expires_at === "number"
          ? input.access_token_expires_at
          : typeof expiresIn === "number"
            ? Date.now() + expiresIn * 1000
            : undefined,
      refresh_token_expires_in: Number.isNaN(
        refreshTokenExpiresIn ?? Number.NaN,
      )
        ? undefined
        : refreshTokenExpiresIn,
      scope: typeof input.scope === "string" ? input.scope : undefined,
      state: typeof input.state === "string" ? input.state : undefined,
    };
  }

  private buildTokenRequest(
    clientId: string,
    code: string,
    codeVerifier?: string,
  ): { headers: Record<string, string>; body: URLSearchParams } {
    const body = new URLSearchParams();
    const headers = this.addClientAuthentication(clientId, body);

    body.set("grant_type", "authorization_code");
    body.set("code", code);
    body.set("redirect_uri", this.getRedirectUri());
    if (codeVerifier) {
      body.set("code_verifier", codeVerifier);
    }

    return { headers, body };
  }

  private addClientAuthentication(
    clientId: string,
    body: URLSearchParams,
  ): Record<string, string> {
    const headers: Record<string, string> = {
      "Content-Type": "application/x-www-form-urlencoded",
    };
    const tokenEndpointAuthMethod =
      this.config.registration?.tokenEndpointAuthMethod;

    if (tokenEndpointAuthMethod === "client_secret_post") {
      body.set("client_id", clientId);
      if (this.config.clientSecret) {
        body.set("client_secret", this.config.clientSecret);
      }
    } else if (
      tokenEndpointAuthMethod === "none" ||
      !this.config.clientSecret
    ) {
      body.set("client_id", clientId);
    } else {
      headers.Authorization = `Basic ${this.encodeBasicAuth(clientId, this.config.clientSecret)}`;
    }

    return headers;
  }

  private encodeBasicAuth(clientId: string, clientSecret: string): string {
    const value = `${clientId}:${clientSecret}`;
    if (typeof btoa === "function") {
      return btoa(value);
    }
    const bufferCtor = (
      globalThis as {
        Buffer?: {
          from(
            data: string,
            encoding: string,
          ): { toString(encoding: string): string };
        };
      }
    ).Buffer;
    if (bufferCtor) {
      return bufferCtor.from(value, "utf8").toString("base64");
    }
    throw new Error("No base64 encoder available for basic auth");
  }

  private isTimeoutError(error: unknown): boolean {
    if (!(error instanceof Error)) {
      return false;
    }
    if (error.name === "AbortError") {
      return true;
    }
    return error.message.toLowerCase().includes("timeout");
  }

  async getUserInfo(): Promise<UserInfo> {
    const tokenResponse = this.getStoredTokenResponse();
    const accessToken = tokenResponse?.access_token;
    if (!accessToken) {
      throw new OidcClientError(
        "NO_ACCESS_TOKEN",
        "Missing access token in stored token response",
      );
    }

    return this.requestUserInfo(accessToken);
  }

  private async requestUserInfo(accessToken: string): Promise<UserInfo> {
    const config = await this.getOidcConfiguration();
    if (!config.userinfo_endpoint) {
      throw new OidcClientError(
        "NO_USERINFO_ENDPOINT",
        "Provider does not support userinfo endpoint",
        { endpoint: this.config.authority },
      );
    }
    const userInfoUrl = new URL(config.userinfo_endpoint);

    if (this.projectOwned) {
      const api = this.ownerApi({
        Authorization: `Bearer ${accessToken}`,
      });
      this.requireOwnerEndpoint(
        userInfoUrl,
        (await api.userinfoRequestOpts()).path,
      );
      const userInfo = UserInfoToJSON(
        await api.userinfo({ redirect: "error" }),
      ) as unknown as UserInfo;
      if (!userInfo || typeof userInfo.sub !== "string")
        throw new OidcClientError(
          "INVALID_USERINFO_RESPONSE",
          "Userinfo response did not include a valid sub claim",
        );
      return userInfo;
    }

    let userInfoResponse: Response;
    try {
      const isNative =
        !this.disableNativeRequests && isNativeProtocol(userInfoUrl);
      if (isNative) {
        userInfoResponse = await this.nativeFetch(userInfoUrl, {
          method: "GET",
          headers: {
            "Content-Type": "application/json;charset=UTF-8",
            Authorization: `Bearer ${accessToken}`,
          },
          credentials: this.config.fetchRequestCredentials ?? "same-origin",
          timeout: this.config.requestTimeoutInSeconds
            ? this.config.requestTimeoutInSeconds * 1000
            : undefined,
        });
      } else {
        userInfoResponse = await this.fetch(userInfoUrl, {
          method: "GET",
          headers: {
            "Content-Type": "application/json;charset=UTF-8",
            Authorization: `Bearer ${accessToken}`,
          },
          credentials: this.config.fetchRequestCredentials ?? "same-origin",
        });
      }
    } catch (error: unknown) {
      if (this.isTimeoutError(error)) {
        throw new OidcClientError(
          "NETWORK_TIMEOUT",
          "Userinfo request timed out",
          {
            endpoint: userInfoUrl.toString(),
            cause: error,
          },
        );
      }
      throw new OidcClientError("NETWORK_ERROR", "Userinfo request failed", {
        endpoint: userInfoUrl.toString(),
        cause: error,
      });
    }
    if (!userInfoResponse.ok) {
      throw new OidcClientError(
        "HTTP_ERROR",
        `Failed userinfo request: ${userInfoResponse.status} ${userInfoResponse.statusText}`,
        {
          status: userInfoResponse.status,
          statusText: userInfoResponse.statusText,
          endpoint: userInfoUrl.toString(),
        },
      );
    }

    let userInfo: UserInfo;
    try {
      userInfo = (await userInfoResponse.json()) as unknown as UserInfo;
    } catch (error: unknown) {
      throw new OidcClientError(
        "JSON_PARSE_ERROR",
        "Failed to parse userinfo response",
        {
          endpoint: userInfoUrl.toString(),
          cause: error,
        },
      );
    }

    if (
      !userInfo ||
      typeof userInfo !== "object" ||
      typeof (userInfo as OidcUserInfo).sub !== "string"
    ) {
      throw new OidcClientError(
        "INVALID_USERINFO_RESPONSE",
        "Userinfo response did not include a valid sub claim",
        {
          endpoint: userInfoUrl.toString(),
        },
      );
    }

    return userInfo;
  }

  private async verifyProjectClient(
    clientId: string,
    accessToken?: string,
  ): Promise<void> {
    if (!this.projectOwned) return;
    if (!accessToken)
      throw new OidcClientError(
        "NO_ACCESS_TOKEN",
        "Token exchange returned no access token",
      );
    const application = this.config.registration?.application as
      | { uri?: string }
      | undefined;
    if (!application?.uri)
      throw new OidcClientError(
        "INVALID_CLIENT_METADATA",
        "Missing expected Application URI",
      );
    const api = this.ownerApi({ Authorization: `Bearer ${accessToken}` });
    const parameters = { clientId };
    this.requireOwnerEndpoint(
      `${this.config.authority}/oauth2/client/${encodeURIComponent(clientId)}`,
      (await api.getClientRequestOpts(parameters)).path,
    );
    const client = await api.getClient(parameters, { redirect: "error" });
    if (
      client?.clientId !== clientId ||
      client?.application?.uri !== application.uri
    )
      throw new OidcClientError(
        "INVALID_CLIENT_METADATA",
        "Client metadata does not match the configured Application",
      );
  }

  async getVerifiedIdentity(): Promise<{
    issuer: string;
    subject: string;
    userInfo: UserInfo;
    accessToken: string;
  }> {
    const context = this.identityContext;
    const active = () => {
      if (context !== this.identityContext)
        throw new OidcClientError(
          "NO_ACCESS_TOKEN",
          "Session changed during identity establishment",
        );
    };
    while (true) {
      active();
      const token = await this.getAccessToken();
      active();
      const generation = this.sessionGeneration;
      const stale = () => {
        active();
        return (
          token !== this.getStoredTokenResponse()?.access_token ||
          generation !== this.sessionGeneration
        );
      };
      try {
        const config = await this.getOidcConfiguration();
        active();
        const userInfo = await this.requestUserInfo(token);
        if (stale()) continue;
        if (!userInfo.sub)
          throw new OidcClientError(
            "INVALID_USERINFO_RESPONSE",
            "Userinfo response did not include a valid sub claim",
          );
        if (this.projectOwned) {
          const clientId = await this.getClientId(config);
          await this.verifyProjectClient(clientId, token);
          if (stale()) continue;
        }
        return {
          issuer: config.issuer,
          subject: userInfo.sub,
          userInfo,
          accessToken: token,
        };
      } catch (error) {
        if (stale()) continue;
        throw error;
      }
    }
  }

  private async requestToken(
    tokenEndpointUrlOrString: URL | string,
    headers: Record<string, string>,
    body: URLSearchParams,
  ): Promise<OidcTokenResponse> {
    const tokenEndpointUrl =
      typeof tokenEndpointUrlOrString === "string"
        ? new URL(tokenEndpointUrlOrString)
        : tokenEndpointUrlOrString;
    if (this.projectOwned) {
      const grantType = body.get("grant_type");
      if (
        !Object.values(TokenGrantTypeEnum).includes(
          grantType as (typeof TokenGrantTypeEnum)[keyof typeof TokenGrantTypeEnum],
        )
      )
        throw new Error("Unsupported owner token grant");
      const parameters: NonNullable<Parameters<DefaultApi["token"]>[0]> = {
        grantType: grantType as NonNullable<
          Parameters<DefaultApi["token"]>[0]
        >["grantType"],
        clientId: body.get("client_id") ?? undefined,
        clientSecret: body.get("client_secret") ?? undefined,
        code: body.get("code") ?? undefined,
        codeVerifier: body.get("code_verifier") ?? undefined,
        redirectUri: body.get("redirect_uri") ?? undefined,
        refreshToken: body.get("refresh_token") ?? undefined,
        scope: body.get("scope") ?? undefined,
      };
      const api = this.ownerApi(headers);
      this.requireOwnerEndpoint(
        tokenEndpointUrl,
        (await api.tokenRequestOpts(parameters)).path,
      );
      return TokenResponseToJSON(
        await api.token(parameters, { redirect: "error" }),
      ) as unknown as OidcTokenResponse;
    }
    const isNative =
      !this.disableNativeRequests && isNativeProtocol(tokenEndpointUrl);

    if (isNative) {
      const response = await this.nativeFetch(tokenEndpointUrl, {
        method: "POST",
        headers,
        body: body.toString(),
        credentials: this.config.fetchRequestCredentials ?? "same-origin",
        timeout: this.config.requestTimeoutInSeconds
          ? this.config.requestTimeoutInSeconds * 1000
          : undefined,
      });
      if (!response.ok) {
        throw new Error(
          `Failed token request: ${response.status} ${response.statusText}`,
        );
      }
      return (await response.json()) as unknown as OidcTokenResponse;
    }

    const response = await this.fetch(tokenEndpointUrl, {
      method: "POST",
      headers,
      body: body.toString(),
      credentials: this.config.fetchRequestCredentials ?? "same-origin",
    });

    if (!response.ok) {
      throw new Error(
        `Failed token request: ${response.status} ${response.statusText}`,
      );
    }

    return (await response.json()) as unknown as OidcTokenResponse;
  }

  async handleSigninCallback(url?: URL): Promise<OidcTokenResponse | null> {
    if (!url) {
      url = new URL(window.location.href);
    }

    const expected = new URL(this.getRedirectUri());
    if (url.origin !== expected.origin || url.pathname !== expected.pathname)
      throw new OidcClientError(
        "INVALID_STATE",
        "Callback destination does not match",
      );
    const queryParams = url.searchParams;
    const hashParams = this.getHashParams(url);

    const callbackError = this.readCallbackParam(
      queryParams,
      hashParams,
      "error",
    );
    if (callbackError) {
      throw new Error("Authorization was rejected by the provider");
    }

    const callbackState = this.readCallbackParam(
      queryParams,
      hashParams,
      "state",
    );

    const accessToken = this.readCallbackParam(
      queryParams,
      hashParams,
      "access_token",
    );
    const idToken = this.readCallbackParam(queryParams, hashParams, "id_token");
    if (accessToken || idToken)
      throw new OidcClientError(
        "INVALID_STATE",
        "Expected an authorization-code callback",
      );

    const authorizationCode = this.readCallbackParam(
      queryParams,
      hashParams,
      "code",
    );
    if (!authorizationCode) {
      return null;
    }

    if (!callbackState) {
      throw new OidcClientError(
        "MISSING_STATE",
        "Signin callback is missing state",
      );
    }

    const codeVerifier = this.consumePkceVerifier(callbackState);
    if (!codeVerifier) {
      throw new OidcClientError(
        "INVALID_STATE",
        "Signin callback state is unknown or has expired",
      );
    }

    this.identityContext += 1;
    const generation = ++this.sessionGeneration;
    const config = await this.getOidcConfiguration();
    const callbackIssuer = this.readCallbackParam(
      queryParams,
      hashParams,
      "iss",
    );
    const callbackClientId = this.readCallbackParam(
      queryParams,
      hashParams,
      "client_id",
    );
    const knownClientId = this.registeredClientId;
    const onboarding = this.projectOwned && !knownClientId;
    if (
      (callbackIssuer !== null && callbackIssuer !== config.issuer) ||
      (onboarding && !callbackIssuer) ||
      (callbackClientId !== null &&
        knownClientId &&
        callbackClientId !== knownClientId) ||
      (onboarding && !callbackClientId?.trim())
    )
      throw new OidcClientError(
        "INVALID_STATE",
        "Callback issuer or client ID does not match",
      );
    const clientId = onboarding
      ? (callbackClientId ?? "")
      : await this.getClientId(config);
    const { headers, body } = this.buildTokenRequest(
      clientId,
      authorizationCode,
      codeVerifier,
    );
    const json = await this.requestToken(config.token_endpoint, headers, body);
    const tokenResponse = this.normalizeTokenResponse({
      ...json,
      state: callbackState ?? json.state,
    });
    if (generation !== this.sessionGeneration)
      throw new OidcClientError(
        "NO_ACCESS_TOKEN",
        "Session ended during callback",
      );
    await this.verifyProjectClient(clientId, tokenResponse.access_token);
    if (generation !== this.sessionGeneration)
      throw new OidcClientError(
        "NO_ACCESS_TOKEN",
        "Session ended during callback",
      );
    if (onboarding) this.rememberIssuedClient(clientId, config.issuer);
    this.rememberTokenResponse(tokenResponse);
    return tokenResponse;
  }

  refreshToken(): Promise<OidcTokenResponse> {
    if (this.refreshPending?.generation === this.sessionGeneration)
      return this.refreshPending.promise;
    const pending = {
      generation: this.sessionGeneration,
      promise: this.performRefresh(),
    };
    this.refreshPending = pending;
    void pending.promise
      .finally(() => {
        if (this.refreshPending === pending) this.refreshPending = null;
      })
      .catch(() => {});
    return pending.promise;
  }

  private async performRefresh(): Promise<OidcTokenResponse> {
    const generation = this.sessionGeneration;
    const storedToken = this.getStoredTokenResponse();
    const snapshot = JSON.stringify(storedToken);
    if (!storedToken?.refresh_token) {
      throw new OidcClientError(
        "NO_REFRESH_TOKEN",
        "Missing refresh token in stored token response",
      );
    }

    const config = await this.getOidcConfiguration();
    const clientId = await this.getClientId(config);
    const body = new URLSearchParams({
      grant_type: "refresh_token",
      refresh_token: storedToken.refresh_token,
    });
    const headers = this.addClientAuthentication(clientId, body);
    const refreshed = this.normalizeTokenResponse({
      ...storedToken,
      access_token: undefined,
      expires_in: undefined,
      access_token_expires_at: undefined,
      ...(await this.requestToken(config.token_endpoint, headers, body)),
    });
    if (
      generation !== this.sessionGeneration ||
      snapshot !== JSON.stringify(this.getStoredTokenResponse())
    ) {
      throw new OidcClientError(
        "NO_ACCESS_TOKEN",
        "Session changed during token refresh",
      );
    }
    await this.verifyProjectClient(clientId, refreshed.access_token);
    if (
      generation !== this.sessionGeneration ||
      snapshot !== JSON.stringify(this.getStoredTokenResponse())
    )
      throw new OidcClientError(
        "NO_ACCESS_TOKEN",
        "Session changed during token refresh",
      );
    this.rememberTokenResponse(refreshed);
    return refreshed;
  }

  async getAccessToken(): Promise<string> {
    const storedToken = this.getStoredTokenResponse();
    if (!storedToken?.access_token && !storedToken?.refresh_token) {
      throw new OidcClientError(
        "NO_ACCESS_TOKEN",
        "Missing access token in stored token response",
      );
    }

    if (
      storedToken.access_token &&
      typeof storedToken.access_token_expires_at === "number" &&
      storedToken.access_token_expires_at > Date.now() + 30_000
    ) {
      return storedToken.access_token;
    }

    if (storedToken.refresh_token) {
      const refreshed = await this.refreshToken();
      if (refreshed.access_token) {
        return refreshed.access_token;
      }
    }

    throw new OidcClientError(
      "NO_ACCESS_TOKEN",
      "Missing refreshed access token",
    );
  }

  static validateConfig(config: OidcClientConfig): OidcClientConfig {
    config = structuredClone(config);
    if (!config.authority || typeof config.authority !== "string") {
      throw new Error("Invalid authority URL");
    }
    const authority = new URL(config.authority);
    if (
      !["http:", "https:"].includes(authority.protocol) ||
      authority.username ||
      authority.password ||
      authority.search ||
      authority.hash
    )
      throw new Error("Invalid authority URL");
    config.authority = authority.href.endsWith("/")
      ? authority.href.slice(0, -1)
      : authority.href;

    if (
      config.redirectUri !== undefined &&
      typeof config.redirectUri !== "string"
    ) {
      throw new Error("Invalid redirectUri");
    }

    if (!OidcClient.resolveRedirectUri(config)) {
      throw new Error(
        "Missing redirectUri: set redirectUri or registration.redirectUris[0]",
      );
    }

    const redirect = new URL(OidcClient.resolveRedirectUri(config) ?? "");
    if (redirect.username || redirect.password || redirect.hash)
      throw new Error("Invalid redirectUri");

    return config;
  }

  private static resolveRedirectUri(
    config: OidcClientConfig,
  ): string | undefined {
    return config.redirectUri ?? config.registration?.redirectUris?.[0];
  }

  private getRedirectUri(): string {
    return OidcClient.resolveRedirectUri(this.config) ?? "";
  }

  private getRegistrationMetadata(): OidcClientMetadata {
    if (!this.config.registration) {
      throw new Error(
        "Missing registration metadata for dynamic client registration",
      );
    }
    return this.config.registration;
  }

  private async fetchOidcConfiguration(): Promise<OidcConfiguration> {
    if (this.projectOwned) {
      return AuthorizationServerMetadataToJSON(
        await this.ownerApi().openidConfiguration({
          redirect: "error",
        }),
      ) as OidcConfiguration;
    }

    const urlString = `${this.config.authority}/.well-known/openid-configuration`;
    const url = new URL(urlString);
    const isNative = !this.disableNativeRequests && isNativeProtocol(url);

    if (isNative) {
      const response = await this.nativeFetch(url, {
        headers: {
          "Content-Type": "application/json;charset=UTF-8",
        },
        timeout: this.config.requestTimeoutInSeconds
          ? this.config.requestTimeoutInSeconds * 1000
          : undefined,
      });
      if (!response.ok) {
        throw new Error(
          `Failed to fetch OIDC configuration: ${response.status} ${response.statusText}`,
        );
      }
      return (await response.json()) as OidcConfiguration;
    }

    const res = await this.fetch(url, {
      credentials: this.config.fetchRequestCredentials ?? "same-origin",
    });
    if (!res.ok) {
      throw new Error(
        `Failed to fetch OIDC configuration: ${res.status} ${res.statusText}`,
      );
    }
    return res.json() as Promise<OidcConfiguration>;
  }

  async getOidcConfiguration(): Promise<OidcConfiguration> {
    if (!this.oidcConfigPromise) {
      this.oidcConfigPromise = this.fetchOidcConfiguration()
        .then((config) => {
          const issuer = new URL(config.issuer);
          if (
            !["http:", "https:"].includes(issuer.protocol) ||
            issuer.username ||
            issuer.password ||
            issuer.search ||
            issuer.hash
          )
            throw new Error("Invalid canonical issuer");
          return config;
        })
        .catch((error) => {
          this.oidcConfigPromise = null;
          throw error;
        });
    }
    return this.oidcConfigPromise;
  }

  async signin(options: SigninOptions = {}): Promise<void> {
    const url = await this.getAuthorizationUrl(options);

    await openUrl(url, {
      popup: options.popup,
      windowFeatures: options.windowFeatures,
    });
  }

  async signout(options: SignoutOptions = {}): Promise<void> {
    this.clearStoredTokenResponse();
    const generation = this.sessionGeneration;
    const config = await this.getOidcConfiguration();
    if (generation !== this.sessionGeneration)
      throw new Error("Session ended during provider logout");
    if (!config.end_session_endpoint)
      throw new Error("Provider does not support logout");
    if (config.end_session_endpoint) {
      const url = new URL(config.end_session_endpoint);
      if (options.extraParams) {
        for (const [k, v] of Object.entries(options.extraParams)) {
          url.searchParams.set(k, String(v));
        }
      }
      if (options.redirectUri) {
        url.searchParams.set("post_logout_redirect_uri", options.redirectUri);
      }

      await openUrl(url, {
        popup: options.popup,
        windowFeatures: options.windowFeatures,
      });
    }
  }

  async getRegistrationUrl(options: RegistrationOptions = {}): Promise<URL> {
    const config = await this.getOidcConfiguration();
    if (!config.registration_endpoint) {
      throw new Error("Provider does not support dynamic client registration");
    }

    const url = new URL(config.registration_endpoint);
    url.searchParams.set(
      "redirect_uri",
      options.redirectUri ?? this.getRedirectUri(),
    );
    if (options.registration) {
      url.searchParams.set(
        "registration",
        JSON.stringify(snakeCaseKeys(options.registration)),
      );
    }
    return url;
  }

  async startRegistration(options: RegistrationOptions = {}): Promise<URL> {
    const generation = this.sessionGeneration;
    const url = await this.getRegistrationUrl(options);
    if (generation !== this.sessionGeneration)
      throw new Error("Session ended during registration");
    await openUrl(url, {
      popup: options.popup,
      windowFeatures: options.windowFeatures,
    });
    return url;
  }

  private async registerClient(config: OidcConfiguration): Promise<string> {
    const endpointUrlString = config.registration_endpoint;
    if (!endpointUrlString) {
      throw new Error(
        "registration_endpoint is required for dynamic client registration",
      );
    }

    const endpointUrl = new URL(endpointUrlString);
    const isNative =
      !this.disableNativeRequests && isNativeProtocol(endpointUrl);

    if (isNative) {
      const nativeFetch = createNativeFetch(endpointUrl);
      const registrationMetadata = this.getRegistrationMetadata();
      endpointUrl.searchParams.set("redirect_uri", this.getRedirectUri());
      endpointUrl.searchParams.set(
        "registration",
        JSON.stringify(snakeCaseKeys(registrationMetadata)),
      );
      const response = await nativeFetch(endpointUrl, {
        headers: {
          "Content-Type": "application/json;charset=UTF-8",
        },
      });
      if (!response.ok) {
        throw new Error(
          `Failed dynamic client registration: ${response.status}`,
        );
      }
      const json =
        (await response.json()) as unknown as OidcClientRegistrationResponse;
      if (!json.client_id) {
        throw new Error("registration response missing client_id");
      }
      this.config.clientId = json.client_id;
      if (json.client_secret) {
        this.config.clientSecret = json.client_secret;
      }
      this.emit("registered", json);
      return json.client_id;
    }

    const res = await this.fetch(endpointUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json;charset=UTF-8" },
      body: JSON.stringify(snakeCaseKeys(this.getRegistrationMetadata())),
    });
    if (!res.ok) {
      throw new Error(`Failed dynamic client registration: ${res.status}`);
    }

    const json =
      (await res.json()) as unknown as OidcClientRegistrationResponse;
    if (!json.client_id) {
      throw new Error("registration response missing client_id");
    }

    this.config.clientId = json.client_id;
    if (json.client_secret) {
      this.config.clientSecret = json.client_secret;
    }
    this.emit("registered", json);
    return json.client_id;
  }

  private async getClientId(config: OidcConfiguration): Promise<string> {
    const issued = this.getIssuedClient();
    if (issued && issued.issuer !== config.issuer)
      throw new OidcClientError(
        "INVALID_STATE",
        "Registered client issuer does not match",
      );
    const clientId = this.registeredClientId;
    if (clientId) return clientId;
    if (this.projectOwned || !config.registration_endpoint) {
      throw new Error("clientId is required for signin");
    }

    return await this.registerClient(config);
  }

  async getAuthorizationUrl(
    options: AuthorizationUrlOptions = {},
  ): Promise<URL> {
    const generation = this.sessionGeneration;
    const config = await this.getOidcConfiguration();
    const clientId =
      this.projectOwned && !this.registeredClientId
        ? undefined
        : await this.getClientId(config);
    const url = new URL(config.authorization_endpoint);

    if (this.config.extraQueryParams) {
      for (const [k, v] of Object.entries(this.config.extraQueryParams)) {
        url.searchParams.set(k, String(v));
      }
    }

    if (options.extraParams) {
      for (const [k, v] of Object.entries(options.extraParams)) {
        url.searchParams.set(k, String(v));
      }
    }

    const responseType =
      this.config.responseType ??
      this.config.registration?.responseTypes?.[0] ??
      "code";

    if (clientId) url.searchParams.set("client_id", clientId);
    else {
      url.searchParams.delete("client_id");
      const application = this.getRegistrationMetadata().application as
        | { uri?: string }
        | undefined;
      if (!application?.uri?.trim())
        throw new Error("Application URI is required for onboarding");
      url.searchParams.set("application_id", application.uri);
    }
    url.searchParams.set("redirect_uri", this.getRedirectUri());
    url.searchParams.set("response_type", responseType);

    if (!this.config.omitScopeWhenRequesting) {
      url.searchParams.set(
        "scope",
        this.config.registration?.scope ?? "openid",
      );
    }

    const state = options.state ?? generateState();
    url.searchParams.set("state", state);

    if (responseType.includes("code")) {
      const pkcePair = await generatePkcePair();
      url.searchParams.set("code_challenge_method", "S256");
      url.searchParams.set("code_challenge", pkcePair.codeChallenge);
      if (generation !== this.sessionGeneration)
        throw new OidcClientError(
          "NO_ACCESS_TOKEN",
          "Session ended during authorization",
        );
      this.rememberPkceVerifier(state, pkcePair.codeVerifier);
    }

    for (const key of OPTIONAL_AUTHORIZATION_PARAMS) {
      const value =
        this.config[key] ??
        (key === "maxAge"
          ? this.config.registration?.defaultMaxAge
          : key === "acrValues"
            ? this.config.registration?.defaultAcrValues?.join(" ")
            : key === "responseMode"
              ? "query"
              : undefined);
      if (value !== undefined && value !== null) {
        url.searchParams.set(snakeCase(key), String(value));
      }
    }

    if (this.config.registration) {
      url.searchParams.set(
        "registration",
        JSON.stringify(
          this.projectOwned
            ? Object.fromEntries(
                Object.entries(snakeCaseKeys(this.config.registration)).filter(
                  ([key]) => key !== "client_id",
                ),
              )
            : snakeCaseKeys(this.config.registration),
        ),
      );
    }

    return url;
  }
}
