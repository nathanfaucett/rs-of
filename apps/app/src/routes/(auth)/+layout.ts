import { FetchError, ResponseError, type UserInfo } from "@ofsdk/idp-client";
import { error, redirect } from "@sveltejs/kit";
import { isTauri } from "@tauri-apps/api/core";
import { resolve } from "$app/paths";
import { afterSigninRedirect } from "$lib/common/state/afterSigninRedirect.svelte";
import { ensureTauriDeviceApiUrl } from "$lib/common/state/deviceClient.svelte";
import { ensureTauriIdpApiUrl, getIdpApiUrl, idpApi } from "$lib/common/state/idpClient.svelte";
import { ensureTauriManagementApiUrl } from "$lib/common/state/managementClient.svelte";
import { nodeSetupStatus } from "$lib/common/state/nodeSetup";
import { clearOidcSession, ensureOidcClientId, getOidcClient } from "$lib/common/state/oidc.svelte";
import { observeOnboarding } from "$lib/common/state/onboarding";
import type { LayoutLoad } from "./$types";

export const prerender = false;

export const load = (async (event) => {
  await event.parent();

  if (!isTauri() && !getIdpApiUrl()) redirect(302, resolve("/setup/installation"));
  const node = await nodeSetupStatus();
  if (node && node.route !== "/signin") redirect(302, node.route);
  const observation = await observeOnboarding(node);
  if (observation.state !== "ready") redirect(302, resolve("/setup/installation"));
  if (isTauri()) {
    await ensureTauriIdpApiUrl();
    await ensureTauriManagementApiUrl();
    await ensureTauriDeviceApiUrl();
  }
  await ensureOidcClientId(observation).catch(() =>
    error(503, "Verified public login Client is unavailable. Retry observation.")
  );

  if (!getIdpApiUrl()) {
    afterSigninRedirect.setURL(event.url);
    redirect(302, resolve("/signin"));
  }

  const oidcClient = getOidcClient();
  const token = oidcClient.getStoredTokenResponse();
  const { iss: tokenIssuer } = token ?? {};
  let issuer: string | undefined;
  if (isTauri() && token?.access_token && tokenIssuer) {
    const base = getIdpApiUrl();
    const metadata = await idpApi
      .openidConfiguration()
      .catch(() =>
        error(
          503,
          "Issuer discovery unavailable. Your session has been preserved. Retry when the selected IdP is reachable."
        )
      );
    if (!metadata.issuer || base !== getIdpApiUrl())
      error(
        502,
        "Selected IdP issuer evidence is unavailable or changed. Your session has been preserved."
      );
    issuer = metadata.issuer;
  }
  if (!token?.access_token || (isTauri() && tokenIssuer && tokenIssuer !== issuer)) {
    clearOidcSession();
    afterSigninRedirect.setURL(event.url);
    redirect(302, resolve("/signin"));
  }

  let userInfo: UserInfo;
  try {
    userInfo = await idpApi.userinfo();
  } catch (cause) {
    if (cause instanceof ResponseError) {
      const status = cause.response.status;
      const body =
        status === 400
          ? await cause.response
              .clone()
              .json()
              .catch(() => null)
          : null;
      if (status === 401 || status === 403 || body?.error === "invalid_grant") {
        clearOidcSession();
        afterSigninRedirect.setURL(event.url);
        redirect(302, resolve("/signin"));
      }
    }
    if (
      cause instanceof FetchError ||
      (cause instanceof ResponseError && cause.response.status >= 500)
    ) {
      error(
        503,
        "Authentication service unavailable. Your session has been preserved. Retry when the service is reachable."
      );
    }
    error(
      502,
      "Unable to verify your session. Your session has been preserved, but access is not granted. Retry to verify it again."
    );
  }
  if (typeof userInfo?.sub !== "string" || !userInfo.sub.trim()) {
    error(
      502,
      "Unable to verify your session. Your session has been preserved, but access is not granted. Retry to verify it again."
    );
  }
  return { userInfo: { ...userInfo } };
}) satisfies LayoutLoad;
