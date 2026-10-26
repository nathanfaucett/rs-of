import {
    type FileSystemResource,
    StorageClient,
    type StorageEntry,
    type StorageRequest,
    type StorageResponse,
    type StorageSocket,
} from "@aicacia/storage-client";

import { env } from "$env/dynamic/public";
import { getIdpApiUrl, getSdk } from "./state/sdk";

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
    const tokenProvider = getSdk().tokenProvider();
    return new StorageClient({
        idpBaseUrl,
        storageBaseUrl,
        clientId: "password-manager-web",
        bearerToken: tokenProvider,
    });
}
