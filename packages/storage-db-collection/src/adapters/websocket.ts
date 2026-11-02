import type { StorageClient } from "@aicacia/storage-client";
import type { ConnectBackend, StorageBackend } from "../adapters/backend.js";

/** Adapts a StorageClient to the StorageBackend interface. */
export function createWebSocketBackend(
    client: StorageClient,
    filesystemId: string,
): ConnectBackend {
    return async (signal?: AbortSignal) => {
        const socket = await client.openSocket(filesystemId, signal);

        const backend: StorageBackend = {
            async read(key: string): Promise<Uint8Array | undefined> {
                const response = await socket.request({
                    type: "read",
                    path: key,
                });
                if (response.type === "read") {
                    return new Uint8Array(response.content);
                }
                return undefined;
            },

            async write(key: string, bytes: Uint8Array): Promise<void> {
                const response = await socket.request({
                    type: "write",
                    path: key,
                    content: [...bytes],
                });
                if (
                    response.type !== "written" ||
                    response.entry.name !== key
                ) {
                    throw new Error(
                        `Write not acknowledged for ${key}: ${response.type}`,
                    );
                }
            },

            async delete(key: string): Promise<void> {
                const response = await socket.request({
                    type: "delete",
                    path: key,
                });
                if (response.type !== "deleted") {
                    throw new Error(
                        `Delete not acknowledged for ${key}: ${response.type}`,
                    );
                }
            },

            async list(prefix: string): Promise<string[]> {
                const response = await socket.request({
                    type: "list",
                    path: prefix,
                });
                if (response.type !== "listed") {
                    throw new Error(
                        `List failed for ${prefix}: ${response.type}`,
                    );
                }
                return response.entries.map((entry) => entry.name);
            },

            close(): void {
                socket.close();
            },

            onDisconnect(handler: () => void): void {
                socket.socket.addEventListener("close", handler, {
                    once: true,
                });
                socket.socket.addEventListener("error", handler, {
                    once: true,
                });
            },
        };

        return backend;
    };
}
