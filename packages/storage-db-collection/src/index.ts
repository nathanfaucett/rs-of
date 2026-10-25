import type { StandardSchemaV1 } from "@standard-schema/spec";
import type { StorageClient, StorageSocket } from "@aicacia/storage-client";
import { createCollection } from "@tanstack/db";

export type StorageCollectionOptions<T extends object> = {
    client: StorageClient;
    userId: string;
    applicationId: string;
    filesystemId: string;
    directory: string;
    schema: StandardSchemaV1<unknown, T>;
    getKey: (record: T) => string;
    extension?: string;
    deserialize?: (content: Uint8Array) => unknown;
};

export function createStorageCollection<T extends object>(
    options: StorageCollectionOptions<T>,
) {
    const {
        client,
        userId,
        applicationId,
        filesystemId,
        directory,
        schema,
        getKey,
    } = options;
    if (!userId || !applicationId || !filesystemId)
        throw new Error(
            "Storage collection requires a User, Application, and Filesystem ID",
        );
    if (!directory.split("/").every(safeSegment))
        throw new Error("Unsafe record directory");
    const extension = options.extension ?? ".json";
    if (!/^\.[a-zA-Z0-9]+$/.test(extension))
        throw new Error("Unsafe record extension");
    const deserialize =
        options.deserialize ??
        ((content: Uint8Array) =>
            JSON.parse(
                new TextDecoder("utf-8", { fatal: true }).decode(content),
            ));
    let disposed = false;
    const collection = createCollection<T, string>({
        id: JSON.stringify([
            userId,
            applicationId,
            filesystemId,
            directory,
            extension,
        ]),
        getKey,
        gcTime: Infinity,
        sync: {
            sync: ({ begin, write, commit, markReady, markError }) => {
                if (disposed) {
                    markError(new Error("Storage collection is disposed"));
                    return;
                }
                const controller = new AbortController();
                let socket: StorageSocket | undefined;
                let failed = false;
                const disconnected = () => {
                    if (!disposed && !failed) {
                        failed = true;
                        socket?.socket.removeEventListener(
                            "close",
                            disconnected,
                        );
                        socket?.socket.removeEventListener(
                            "error",
                            disconnected,
                        );
                        socket?.close();
                        markError(
                            new Error(
                                "Storage connection closed; reopen the filesystem",
                            ),
                        );
                    }
                };
                void (async () => {
                    try {
                        const opened = await client.openSocket(
                            filesystemId,
                            controller.signal,
                        );
                        if (disposed) {
                            opened.close();
                            return;
                        }
                        socket = opened;
                        socket.socket.addEventListener("close", disconnected);
                        socket.socket.addEventListener("error", disconnected);
                        let parent = "";
                        for (const segment of directory.split("/")) {
                            const listing = await socket.request({
                                type: "list",
                                path: parent,
                            });
                            if (disposed || failed) return;
                            if (listing.type !== "listed")
                                throw new Error(
                                    "Storage directory listing failed",
                                );
                            parent = parent ? `${parent}/${segment}` : segment;
                            if (
                                !listing.entries.some(
                                    (entry) => entry.name === parent,
                                )
                            ) {
                                markReady();
                                return;
                            }
                        }
                        const response = await socket.request({
                            type: "list",
                            path: directory,
                        });
                        if (disposed || failed) return;
                        if (response.type !== "listed")
                            throw new Error("Storage record listing failed");
                        const records: T[] = [];
                        for (const entry of response.entries) {
                            if (!entry.name.endsWith(extension)) continue;
                            const prefix = `${directory}/`;
                            const key = entry.name.slice(
                                prefix.length,
                                -extension.length,
                            );
                            if (
                                !entry.name.startsWith(prefix) ||
                                !safeSegment(key)
                            )
                                throw new Error("Unsafe record key");
                            const read = await socket.request({
                                type: "read",
                                path: `${directory}/${key}${extension}`,
                            });
                            if (disposed || failed) return;
                            if (read.type !== "read")
                                throw new Error("Storage record read failed");
                            let record: T;
                            try {
                                const result = await schema[
                                    "~standard"
                                ].validate(
                                    deserialize(new Uint8Array(read.content)),
                                );
                                if (disposed || failed) return;
                                if (!("value" in result))
                                    throw new Error("Invalid record");
                                record = result.value;
                            } catch {
                                throw new Error("Malformed Storage record");
                            }
                            if (
                                getKey(record) !== key ||
                                records.some((value) => getKey(value) === key)
                            )
                                throw new Error(
                                    "Storage record identity mismatch",
                                );
                            records.push(record);
                        }
                        if (disposed || failed) return;
                        begin();
                        for (const value of records)
                            write({ type: "insert", value });
                        await commit();
                        if (!disposed && !failed) markReady();
                    } catch (cause) {
                        if (disposed || failed) return;
                        socket?.socket.removeEventListener(
                            "close",
                            disconnected,
                        );
                        socket?.socket.removeEventListener(
                            "error",
                            disconnected,
                        );
                        socket?.close();
                        markError(
                            cause instanceof Error
                                ? cause
                                : new Error("Storage initial load failed"),
                        );
                    }
                })();
                return () => {
                    disposed = true;
                    controller.abort();
                    socket?.socket.removeEventListener("close", disconnected);
                    socket?.socket.removeEventListener("error", disconnected);
                    socket?.close();
                };
            },
        },
    });
    collection.on("status:change", ({ status }) => {
        if (status === "cleaned-up") disposed = true;
    });
    return collection;
}

function safeSegment(value: string): boolean {
    return /^[a-zA-Z0-9_-][a-zA-Z0-9_.-]*$/.test(value) && !value.endsWith(".");
}
