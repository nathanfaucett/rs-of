import { Configuration, type ConfigurationParameters, DefaultApi } from "@ofsdk/management-client";
import { createStorage } from "@ofsdk/svelte-headless";
import { isTauri } from "@tauri-apps/api/core";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { page } from "$app/state";
import { env } from "$env/dynamic/public";
import { afterSigninRedirect } from "./afterSigninRedirect.svelte";
import { ownerAccessToken, usesRemoteApis } from "./idpClient.svelte";
import { ensureLocalhostBaseUrl } from "./localhostBaseUrl.svelte";
import { validateServiceApiUrl } from "./serviceApi";

const managementApiUrl = createStorage<string | null>(
  "management-api-url",
  env.PUBLIC_MANAGEMENT_BASE_URL ?? null
);

function readAccessToken(): Promise<string> {
  return ownerAccessToken("idp-management");
}

function readBasePath(): string | undefined {
  const basePath = managementApiUrl.item;
  return basePath === null ? undefined : basePath;
}

const defaultConfigurationParameters: ConfigurationParameters = {
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
  accessToken(): Promise<string> {
    return readAccessToken();
  },
  get basePath(): string | undefined {
    return readBasePath();
  },
  get fetchApi() {
    return fetch;
  },
  credentials: "same-origin",
};

export const lidpManagementConfiguration = new Configuration(defaultConfigurationParameters);

export const managementApi = new DefaultApi(lidpManagementConfiguration);

export function setManagementApiUrl(newManagementApiUrl: string) {
  managementApiUrl.item = newManagementApiUrl;
}

export async function ensureTauriManagementApiUrl(): Promise<void> {
  if (isTauri() && !usesRemoteApis())
    managementApiUrl.item = `${await ensureLocalhostBaseUrl()}/management`;
}

export function getManagementApiUrl(): string | null {
  return managementApiUrl.item;
}

export async function validateManagementApiUrl(basePath: string): Promise<boolean> {
  return validateServiceApiUrl(basePath, "management");
}
