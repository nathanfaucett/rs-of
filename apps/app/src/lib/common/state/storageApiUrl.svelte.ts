import { createStorage } from "@aicacia/svelte-headless";
import { env } from "$env/dynamic/public";
import { fetchSetupStatus } from "./setupApi";

const storageApiUrl = createStorage<string | null>(
    "storage-api-url",
    env.PUBLIC_STORAGE_BASE_URL ?? null,
);

export function getStorageApiUrl(): string | null {
    return storageApiUrl.item;
}
export function setStorageApiUrl(base: string): void {
    storageApiUrl.item = base;
}
export async function validateStorageApiUrl(base: string): Promise<boolean> {
    try {
        return (await fetchSetupStatus(base)).owners[0] === "storage";
    } catch {
        return false;
    }
}
