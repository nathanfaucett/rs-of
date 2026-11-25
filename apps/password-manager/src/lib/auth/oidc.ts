import { OidcClient } from "@ofsdk/oidc-client";

export interface OidcAuthConfig {
  idp: string;
  storage: string;
  clientId?: string;
  redirectUri: string;
  scopes: string;
  applicationUri: string;
}

export function createOidcClient(config: OidcAuthConfig): OidcClient {
  const { idp, clientId, redirectUri, scopes, applicationUri } = config;
  const fixedClient = clientId === undefined ? {} : { clientId };
  return new OidcClient({
    projectOwned: true,
    clientConfig: {
      authority: idp,
      ...fixedClient,
      redirectUri,
      responseType: "code",
      requestTimeoutInSeconds: 10,
      registration: {
        application: { uri: applicationUri },
        ...fixedClient,
        clientName: "Password Manager",
        scope: scopes,
        redirectUris: [redirectUri],
        postLogoutRedirectUris: [redirectUri.replace(/\/callback$/, "")],
        grantTypes: clientId
          ? ["authorization_code", "refresh_token"]
          : [
              "authorization_code",
              "refresh_token",
              "urn:ietf:params:oauth:grant-type:token-exchange",
            ],
        responseTypes: ["code"],
        tokenEndpointAuthMethod: "none",
        clientType: "public",
        profile: "web_application",
      },
    },
    storage: "local",
    disableNativeRequests: true,
  });
}
