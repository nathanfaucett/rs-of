import {
    type FileSystemResource,
    StorageClient,
    type StorageEntry,
    type StorageRequest,
    type StorageResponse,
    type StorageSocket,
} from "@aicacia/storage-client";

import { env } from "$env/dynamic/public";
import { getIdpApiUrl } from "./state/idpClient.svelte";
import { getOidcClient } from "./state/oidc.svelte";

export type {
    FileSystemResource,
    StorageEntry,
    StorageRequest,
    StorageResponse,
    StorageSocket,
};

export function createStorageClient(): StorageClient {
    const idpBaseUrl = getIdpApiUrl();
    if (!idpBaseUrl) throw new Error("IdP API URL is not configured");
    const storageBaseUrl = env.PUBLIC_STORAGE_BASE_URL;
    if (!storageBaseUrl) throw new Error("Storage API URL is not configured");
    const oidc = getOidcClient();
    return new StorageClient({
        idpBaseUrl,
        storageBaseUrl,
        clientId: "password-manager-web",
        bearerToken: () => oidc.getAccessToken(),
    });
}
