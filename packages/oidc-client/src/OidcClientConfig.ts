import type { OidcClientMetadata } from "./OidcClientMetadata.js";

export type OidcClientConfig = {
  /**
   * Base URL of the OIDC provider (issuer).
   */
  authority: string;
  /**
   * OpenID Connect client metadata.
   */
  registration: OidcClientMetadata;
  /**
   * Client identifier issued by the provider.
   */
  clientId?: string;
  /**
   * Client secret for confidential clients.
   */
  clientSecret?: string;
  /**
   * Expected response_type for authorization requests (e.g. "code", "id_token", "token").
   */
  responseType?: string;
  /**
   * Redirect URI where the provider will send responses.
   * If omitted, the first entry in registration.redirectUris is used.
   */
  redirectUri?: string;
  /**
   * Prompt parameter to control UI (login, consent, etc.).
   */
  prompt?: string;
  /**
   * Display hint for the authorization UI.
   */
  display?: string;
  /**
   * Maximum allowed authentication age in seconds.
   */
  maxAge?: number;
  /**
   * Desired UI languages.
   */
  uiLocales?: string;
  /**
   * Requested ACR values.
   */
  acrValues?: string;
  /**
   * Resource or resources being requested.
   */
  resource?: string | string[];
  /**
   * How the authorization response is returned (query or fragment).
   */
  responseMode?: "query" | "fragment";
  /**
   * Additional query parameters for authorization requests.
   */
  extraQueryParams?: Record<string, string | number | boolean>;
  /**
   * Credentials option passed to fetch calls.
   */
  fetchRequestCredentials?: RequestCredentials;
  /**
   * Timeout for network requests in seconds.
   */
  requestTimeoutInSeconds?: number;
  /**
   * Omit the scope parameter when making requests.
   */
  omitScopeWhenRequesting?: boolean;
};
