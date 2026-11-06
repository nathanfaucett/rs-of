import { StorageClient } from "@ofsdk/storage-client";
import { createStorage } from "@ofsdk/svelte-headless";
import * as v from "valibot";
import { listDevices } from "./devices.svelte";
import { getIdpApiUrl } from "./idpClient.svelte";
import { getOidcClient } from "./oidc.svelte";
import { getStorageApiUrl } from "./storageApiUrl.svelte";

const target = createStorage<string | null>("sync-target-url", null);
export const directions = ["Push", "Pull", "Bidirectional"] as const;
export type SyncDirection = (typeof directions)[number];
const preferencesSchema = v.object({
    deviceIds: v.array(v.string()),
    direction: v.picklist(directions),
    results: v.record(
        v.string(),
        v.object({
            checkedAt: v.string(),
            direction: v.picklist(directions),
            status: v.literal("blocked"),
        }),
    ),
});
export type SyncPreferences = v.InferOutput<typeof preferencesSchema>;
export const ownerBlocker =
    "No compatible local owner binding or authenticated native sync command is available. SQL, KV, and file content transfer are not qualified. No data was transferred.";

export function syncTarget(): string | null {
    return target.item ?? getStorageApiUrl();
}
export function validateSyncTarget(value: string): string {
    const url = new URL(value);
    if (
        url.protocol !== "https:" ||
        url.username ||
        url.password ||
        url.search ||
        url.hash
    )
        throw new Error(
            "Use an HTTPS Storage API URL without credentials, query, or fragment.",
        );
    return url.href.replace(/\/+$/, "");
}
export function saveSyncTarget(value: string | null): void {
    target.item = value === null ? null : validateSyncTarget(value);
}
export async function syncContext() {
    const client = getOidcClient();
    const base = getIdpApiUrl();
    if (!client?.getStoredTokenResponse()?.access_token || !base)
        throw new Error("Sign in before checking sync.");
    const identity = await client.getVerifiedIdentity();
    const token = identity.accessToken;
    const source = getStorageApiUrl();
    const destination = syncTarget();
    const assertCurrent = () => {
        if (
            getIdpApiUrl() !== base ||
            getOidcClient() !== client ||
            client.getStoredTokenResponse()?.access_token !== token ||
            getStorageApiUrl() !== source ||
            syncTarget() !== destination
        )
            throw new Error("Session changed. Check sync again.");
    };
    assertCurrent();
    const scope = JSON.stringify([
        identity.issuer,
        identity.subject,
        source,
        destination,
    ]);
    const stored = createStorage<SyncPreferences>(`sync-preferences:${scope}`, {
        deviceIds: [],
        direction: "Push",
        results: {},
    });
    const parsed = v.safeParse(preferencesSchema, stored.item);
    if (!parsed.success)
        stored.item = { deviceIds: [], direction: "Push", results: {} };
    return {
        stored,
        assertCurrent,
        async discover(signal: AbortSignal) {
            signal.throwIfAborted();
            assertCurrent();
            if (!source || !client.registeredClientId)
                throw new Error(
                    "Configure the source Storage API and sign in with its Application Client.",
                );
            const storage = new StorageClient({
                idpBaseUrl: base,
                storageBaseUrl: source,
                clientId: client.registeredClientId,
                bearerToken: () => {
                    assertCurrent();
                    return token;
                },
                fetch: (url, init) => {
                    assertCurrent();
                    return fetch(url, { ...init, redirect: "error" });
                },
            });
            const [databases, filesystems, devices] = await Promise.all([
                storage.listDatabases(signal),
                storage.listFileSystems(signal),
                listDevices(signal),
            ]);
            signal.throwIfAborted();
            assertCurrent();
            return {
                resources: [
                    ...databases.map((resource) => ({
                        ...resource,
                        kind: "Database",
                    })),
                    ...filesystems.map((resource) => ({
                        ...resource,
                        kind: "Filesystem",
                    })),
                ],
                devices: devices.filter(
                    (device) =>
                        device.state === "approved" && device.revokedAt == null,
                ),
            };
        },
    };
}
