import { createStorageCollection } from "@aicacia/storage-db-collection";
import type { StorageAccess } from "@of/sdk";
import { SecretSchema } from "$lib/models/secret";

export function createSecretCollection(options: {
    client: StorageAccess;
    userId: string;
    filesystemId: string;
}) {
    return createStorageCollection({
        ...options,
        applicationId: "password-manager.aicacia.com",
        directory: "secrets",
        schema: SecretSchema,
        getKey: (secret) => secret.id,
    });
}
