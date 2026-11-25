import type { OidcClient } from "@ofsdk/oidc-client";

export class SessionExpiredError extends Error {
  constructor() {
    super("Your session expired. Sign in again.");
  }
}

export async function requireSessionAccessToken(
  client: Pick<OidcClient, "getStoredTokenResponse" | "getAccessToken">,
  isCurrentClient: () => boolean,
  clear: () => void
): Promise<string> {
  const previous = client.getStoredTokenResponse();
  if (previous?.access_token || previous?.refresh_token) {
    const snapshot = JSON.stringify(previous);
    let accessToken: string;
    try {
      accessToken = await client.getAccessToken();
    } catch {
      if (isCurrentClient() && snapshot === JSON.stringify(client.getStoredTokenResponse())) {
        clear();
        throw new SessionExpiredError();
      }
      throw new Error("Sign-in session changed");
    }
    if (isCurrentClient() && client.getStoredTokenResponse()?.access_token === accessToken)
      return accessToken;
  }
  throw new Error("Sign-in session changed");
}
