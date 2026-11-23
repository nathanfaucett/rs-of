import type { AuthorizationRequest } from "@ofsdk/idp-client";
import { idpApi } from "$lib/common/state/idpClient.svelte";
import { redirectToUrl } from "$lib/common/util/redirectToUrl";

function authorizationRedirectUrl(
  authorizationRequest: Pick<AuthorizationRequest, "redirectUri">
): URL {
  if (!authorizationRequest.redirectUri) {
    throw new Error("Authorization request did not include a redirect URI");
  }
  return new URL(authorizationRequest.redirectUri);
}

export async function rejectAuthorizationRequest(
  authorizationRequest: Pick<AuthorizationRequest, "redirectUri" | "state" | "nonce">,
  error: string,
  errorDescription: string
) {
  const url = authorizationRedirectUrl(authorizationRequest);
  if (authorizationRequest.state) {
    url.searchParams.set("state", authorizationRequest.state);
  }
  if (authorizationRequest.nonce) {
    url.searchParams.set("nonce", authorizationRequest.nonce);
  }
  url.searchParams.set("error", error);
  url.searchParams.set("error_description", errorDescription);
  await redirectToUrl(url);
}

export async function resolveAuthorizationRequest(authorizationRequest: AuthorizationRequest) {
  const authorizeResponse = await idpApi.authorizeJson({
    authorizationRequest,
  });

  if ("error" in authorizeResponse) {
    throw new Error(authorizeResponse.errorDescription ?? authorizeResponse.error);
  }

  if (!authorizeResponse.code) {
    throw new Error("authorization code response did not include code");
  }

  const url = authorizationRedirectUrl(authorizationRequest);
  url.searchParams.set("code", authorizeResponse.code);
  url.searchParams.set("client_id", authorizationRequest.clientId);
  if (authorizeResponse.iss) {
    url.searchParams.set("iss", authorizeResponse.iss);
  }
  const state = authorizeResponse.state ?? authorizationRequest.state;
  if (state) {
    url.searchParams.set("state", state);
  }
  if (authorizationRequest.nonce) {
    url.searchParams.set("nonce", authorizationRequest.nonce);
  }

  return await redirectToUrl(url);
}
