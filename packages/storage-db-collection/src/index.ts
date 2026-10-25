import type { StandardSchemaV1 } from "@standard-schema/spec";
import type { StorageClient, StorageSocket } from "@aicacia/storage-client";
import { createCollection, type MutationFnParams } from "@tanstack/db";

export class UnacknowledgedStorageWriteError extends Error {
    constructor() {
        super(
            "Storage write was not acknowledged; authoritative records reloaded. Verify the outcome before retrying",
        );
        this.name = "UnacknowledgedStorageWriteError";
    }
}

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
    serialize?: (record: T) => Uint8Array;
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
    const serialize =
        options.serialize ??
        ((record: T) => new TextEncoder().encode(JSON.stringify(record)));
    let disposed = false;
    let failed = false;
    let ready = false;
    let socket: StorageSocket | undefined;
    let persist: ((params: MutationFnParams<T>) => Promise<void>) | undefined;
    let queue = Promise.resolve();

    function active() {
        if (disposed)
            throw new Error(
                "Storage collection is disposed; a sent write may still persist",
            );
        if (failed || !socket)
            throw new Error(
                "Storage connection unavailable; reopen and verify records before retrying",
            );
        return socket;
    }
    function path(key: string) {
        if (!safeSegment(key)) throw new Error("Unsafe record key");
        return `${directory}/${key}${extension}`;
    }
    async function validate(input: unknown, key: string): Promise<T> {
        path(key);
        let record: T;
        try {
            const result = await schema["~standard"].validate(input);
            active();
            if (result.issues || !("value" in result))
                throw new Error("Invalid record");
            record = result.value;
        } catch {
            active();
            throw new Error("Malformed Storage record");
        }
        if (getKey(record) !== key)
            throw new Error("Storage record identity mismatch");
        return record;
    }
    async function snapshot(): Promise<T[]> {
        const connection = active();
        let parent = "";
        for (const segment of directory.split("/")) {
            const listing = await connection.request({
                type: "list",
                path: parent,
            });
            active();
            if (listing.type !== "listed")
                throw new Error("Storage directory listing failed");
            parent = parent ? `${parent}/${segment}` : segment;
            if (!listing.entries.some((entry) => entry.name === parent))
                return [];
        }
        const response = await connection.request({
            type: "list",
            path: directory,
        });
        active();
        if (response.type !== "listed")
            throw new Error("Storage record listing failed");
        const records: T[] = [];
        for (const entry of response.entries) {
            if (!entry.name.endsWith(extension)) continue;
            const prefix = `${directory}/`;
            const key = entry.name.slice(prefix.length, -extension.length);
            if (!entry.name.startsWith(prefix))
                throw new Error("Unsafe record key");
            const read = await connection.request({
                type: "read",
                path: path(key),
            });
            active();
            if (read.type !== "read")
                throw new Error("Storage record read failed");
            let input: unknown;
            try {
                input = deserialize(new Uint8Array(read.content));
            } catch {
                throw new Error("Malformed Storage record");
            }
            const record = await validate(input, key);
            if (records.some((value) => getKey(value) === key))
                throw new Error("Storage record identity mismatch");
            records.push(record);
        }
        active();
        return records;
    }
    function mutate(params: MutationFnParams<T>): Promise<void> {
        const result = queue.then(async () => {
            active();
            if (!ready || !persist)
                throw new Error("Storage collection is not ready");
            await persist(params);
        });
        // Wait for core settlement before starting the next Storage operation.
        queue = result
            .then(
                () => params.transaction.isPersisted.promise,
                () => params.transaction.isPersisted.promise,
            )
            .then(
                () => undefined,
                () => undefined,
            );
        return result;
    }
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
        onInsert: mutate,
        onUpdate: mutate,
        onDelete: mutate,
        sync: {
            sync: ({
                begin,
                write,
                commit,
                truncate,
                markReady,
                markError,
            }) => {
                if (disposed) {
                    markError(new Error("Storage collection is disposed"));
                    return;
                }
                const controller = new AbortController();
                const detach = () => {
                    socket?.socket.removeEventListener("close", disconnected);
                    socket?.socket.removeEventListener("error", disconnected);
                };
                const fail = (error: Error) => {
                    if (disposed || failed) return;
                    failed = true;
                    detach();
                    socket?.close();
                    markError(error);
                };
                const disconnected = () =>
                    fail(
                        new Error(
                            "Storage connection closed; a sent write may have persisted. Reopen and verify records before retrying",
                        ),
                    );
                const publish = (records: T[]) => {
                    active();
                    begin();
                    truncate();
                    for (const value of records)
                        write({ type: "insert", value });
                    return commit();
                };
                persist = async ({ transaction }) => {
                    const [mutation] = transaction.mutations;
                    if (!mutation || transaction.mutations.length !== 1)
                        throw new Error(
                            "Storage supports single-record mutations only; no atomic batches",
                        );

                    const key = String(mutation.key);
                    const filename = path(key);
                    let bytes: Uint8Array | undefined;
                    if (mutation.type !== "delete") {
                        const record = await validate(mutation.modified, key);
                        try {
                            bytes = serialize(record);
                        } catch {
                            throw new Error(
                                "Storage record serialization failed",
                            );
                        }
                    }
                    const connection = active();
                    if (transaction.state !== "persisting")
                        throw new Error(
                            "Storage mutation was rolled back before sending",
                        );
                    let outcome: "acknowledged" | "rejected" | "ambiguous" =
                        "ambiguous";
                    try {
                        const response = await connection.request(
                            bytes
                                ? {
                                      type: "write",
                                      path: filename,
                                      content: [...bytes],
                                  }
                                : { type: "delete", path: filename },
                        );
                        active();
                        if (response.type === "error") outcome = "rejected";
                        else if (
                            mutation.type === "delete"
                                ? response.type === "deleted"
                                : response.type === "written" &&
                                  response.entry.name === filename
                        )
                            outcome = "acknowledged";
                    } catch {
                        active();
                    }
                    try {
                        const records = await snapshot();
                        // Core accepts this sync now but holds visibility until persisting
                        // transactions settle. Awaiting the receipt here would deadlock.
                        const receipt = publish(records);
                        if (receipt !== true)
                            void receipt.catch(() =>
                                fail(
                                    new Error(
                                        "Storage reconciliation failed; reopen and verify records before retrying",
                                    ),
                                ),
                            );
                    } catch {
                        const error = new Error(
                            "Storage write outcome could not be reconciled; it may have persisted. Reopen and verify records before retrying",
                        );
                        fail(error);
                        throw error;
                    }
                    if (outcome === "ambiguous")
                        throw new UnacknowledgedStorageWriteError();
                    if (outcome === "rejected")
                        throw new Error(
                            "Storage write rejected; authoritative records reloaded. Review before retrying",
                        );
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
                        const records = await snapshot();
                        await publish(records);
                        if (!disposed && !failed) {
                            ready = true;
                            markReady();
                        }
                    } catch (cause) {
                        fail(
                            cause instanceof Error
                                ? cause
                                : new Error("Storage initial load failed"),
                        );
                    }
                })();
                return () => {
                    disposed = true;
                    controller.abort();
                    detach();
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
