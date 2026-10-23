import {
  Configuration,
  type ConfigurationParameters,
  DefaultApi,
} from "@aicacia/device-client";
import { createStorage } from "@aicacia/svelte-headless";
import { isTauri } from "@tauri-apps/api/core";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { page } from "$app/state";
import { env } from "$env/dynamic/public";
import { afterSigninRedirect } from "./afterSigninRedirect.svelte";
import { ensureLocalhostBaseUrl } from "./localhostBaseUrl.svelte";
import { ownerAccessToken, usesRemoteApis } from "./idpClient.svelte";
import { fetchSetupStatus } from "./setupApi";

const deviceApiUrl = createStorage<string | null>(
  "device-api-url",
  env.PUBLIC_DEVICE_BASE_URL ?? null,
);

function readAccessToken(): Promise<string> {
  return ownerAccessToken("idp-management");
}

function readBasePath(): string | undefined {
  const basePath = deviceApiUrl.item;
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

export const lidpDeviceConfiguration = new Configuration(
  defaultConfigurationParameters,
);

export const deviceApi = new DefaultApi(lidpDeviceConfiguration);

export function setDeviceApiUrl(newDeviceApiUrl: string) {
  deviceApiUrl.item = newDeviceApiUrl;
}

export async function ensureTauriDeviceApiUrl(): Promise<void> {
  if (isTauri() && !usesRemoteApis())
    deviceApiUrl.item = `${await ensureLocalhostBaseUrl()}/device`;
}

export function getDeviceApiUrl(): string | null {
  return deviceApiUrl.item;
}

export async function validateDeviceApiUrl(
  basePath: string,
): Promise<boolean> {
  if (!basePath) {
    return false;
  }
  const configuration = new Configuration({
    ...defaultConfigurationParameters,
    basePath,
  });
  const api = new DefaultApi(configuration);

  try {
    const version = await api.version();
    if (version.name === "device-server") return true;
  } catch {}
  try {
    return (await fetchSetupStatus(basePath)).owners.includes("device");
  } catch {
    return false;
  }
}
