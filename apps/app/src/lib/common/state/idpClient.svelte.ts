import {
  Configuration,
  type ConfigurationParameters,
  DefaultApi,
  SubjectTokenType,
  TokenGrantTypeEnum,
} from "@ofsdk/idp-client";
import { createStorage } from "@ofsdk/svelte-headless";
import { isTauri } from "@tauri-apps/api/core";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { page } from "$app/state";
import { env } from "$env/dynamic/public";
import { afterSigninRedirect } from "./afterSigninRedirect.svelte";
import { getDeviceApiUrl } from "./deviceClient.svelte";
import { ensureLocalhostBaseUrl, getLocalhostBaseUrlCached } from "./localhostBaseUrl.svelte";
import { getOidcClient } from "./oidc.svelte";
import { validateServiceApiUrl } from "./serviceApi";

const idpApiUrl = createStorage<string | null>("idp-api-url", env.PUBLIC_IDP_BASE_URL ?? null);

function readAccessToken(): string {
  const oidcClient = getOidcClient();
  if (!oidcClient) {
    return "";
  }
  return oidcClient.getStoredTokenResponse()?.access_token ?? "";
}

function readBasePath(): string | undefined {
  const basePath = idpApiUrl.item;
  return basePath === null ? undefined : basePath;
}

export const defaultConfigurationParameters: ConfigurationParameters = {
  middleware: [
    {
      pre: async (context) => ({
        ...context,
        init: {
          ...context.init,
          mode: "cors",
        },
      }),
    },
    {
      post: async (context) => {
        if (context.response.status === 401) {
          afterSigninRedirect.setURL(page.url);
          await goto(resolve("/signin"));
        }
        return context.response;
      },
    },
  ],
  get fetchApi() {
    return fetch;
  },
  async accessToken(_name, _scopes): Promise<string> {
    return idpOwnerAccessToken();
  },
  get basePath(): string | undefined {
    return readBasePath();
  },
  credentials: "same-origin",
};

export const lidpConfiguration = new Configuration(defaultConfigurationParameters);

export const idpApi = new DefaultApi(lidpConfiguration);

export function idpApiForFetch(fetchApi: typeof fetch): DefaultApi {
  return new DefaultApi(
    new Configuration({
      ...defaultConfigurationParameters,
      get basePath() {
        return readBasePath();
      },
      fetchApi,
      accessToken: () => idpOwnerAccessToken(fetchApi),
    })
  );
}

export async function passwordToken(clientId: string, username: string, password: string) {
  const base = await ensureTauriIdpApiUrl();
  if (!base) throw new Error("An IdP endpoint is required");
  const owner = new DefaultApi(
    new Configuration({
      basePath: base.replace(/\/$/, ""),
      fetchApi: fetch,
    })
  );
  return owner.token(
    {
      grantType: "password",
      clientId,
      username,
      password,
      scope: "openid profile email",
    },
    { redirect: "error" }
  );
}

export async function ownerAccessToken(
  resource: string,
  fetchApi: typeof fetch = fetch
): Promise<string> {
  const base = await ensureTauriIdpApiUrl(fetchApi);
  const subjectToken = readAccessToken();
  if (!subjectToken) return "";
  if (
    !base ||
    (resource !== base && resource !== "idp-management" && resource !== getDeviceApiUrl())
  )
    throw new Error("A current IdP, Device or Management owner audience is required");
  const owner = new DefaultApi(new Configuration({ basePath: base, fetchApi }));
  const token = await owner.token({
    grantType: TokenGrantTypeEnum.UrnIetfParamsOauthGrantTypeTokenExchange,
    subjectToken,
    subjectTokenType: SubjectTokenType.UrnIetfParamsOauthTokenTypeAccessToken,
    resource,
  });
  return token.accessToken;
}

export async function idpOwnerAccessToken(fetchApi: typeof fetch = fetch): Promise<string> {
  const base = await ensureTauriIdpApiUrl(fetchApi);
  if (!base) throw new Error("An IdP endpoint is required");
  return ownerAccessToken(base, fetchApi);
}

export function setIdpApiUrl(newIdpApiUrl: string) {
  idpApiUrl.item = newIdpApiUrl;
}
export function usesRemoteApis(): boolean {
  if (!isTauri()) return true;
  const base = idpApiUrl.item?.replace(/\/$/, "");
  const local = getLocalhostBaseUrlCached();
  return Boolean(env.PUBLIC_IDP_BASE_URL || (base && (!local || base !== `${local}/idp`)));
}

export function getIdpApiUrl(): string | null {
  return idpApiUrl.item;
}

export async function ensureTauriIdpApiUrl(fetchApi: typeof fetch = fetch): Promise<string | null> {
  if (usesRemoteApis()) {
    return idpApiUrl.item;
  }

  const baseUrl = await ensureLocalhostBaseUrl(fetchApi);
  idpApiUrl.item = `${baseUrl}/idp`;
  return idpApiUrl.item;
}

export async function validateIdpApiUrl(basePath: string): Promise<boolean> {
  return validateServiceApiUrl(basePath, "idp");
}
