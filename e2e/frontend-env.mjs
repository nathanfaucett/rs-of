import { appURL, backendURL, passwordManagerURL } from "./backend.mjs";

export const frontendEnv = {
  PUBLIC_IDP_BASE_URL: `${backendURL}/idp`,
  PUBLIC_DEVICE_BASE_URL: `${backendURL}/device`,
  PUBLIC_STORAGE_BASE_URL: `${backendURL}/storage`,
  PUBLIC_STORAGE_URL: `${backendURL}/storage`,
  PUBLIC_OIDC_CLIENT_ID: "password-manager",
  PUBLIC_OIDC_REDIRECT_URI: `${passwordManagerURL}/callback`,
  PUBLIC_URL: appURL,
};
