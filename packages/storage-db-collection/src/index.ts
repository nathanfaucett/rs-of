import type { StandardSchemaV1 } from "@standard-schema/spec";
import type {
    StorageConnection,
    StorageRequest,
} from "@aicacia/storage-client";
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
    client: {
        openSocket(
            filesystemId: string,
            signal?: AbortSignal,
        ): Promise<StorageConnection>;
    };
    userId: string;
    applicationId: string;
    filesystemId: string;
    directory: string;
    schema: StandardSchemaV1<unknown, T>;
    getKey: (record: T) => string;
    extension?: string;
    deserialize?: (content: Uint8Array) => unknown;
    serialize?: (record: T) => Uint8Array;
    requestTimeoutMs?: number;
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
    const requestTimeoutMs = options.requestTimeoutMs ?? 30_000;
    if (
        !Number.isInteger(requestTimeoutMs) ||
        requestTimeoutMs <= 0 ||
        requestTimeoutMs > 2_147_483_647
    )
        throw new Error(
            "Storage request timeout must be a positive timer-safe integer",
        );
    async function deadline<R>(
        pending: Promise<R>,
        expire: (error: Error) => void,
    ): Promise<R> {
        let timer: ReturnType<typeof setTimeout> | undefined;
        try {
            return await Promise.race([
                pending,
                new Promise<never>((_, reject) => {
                    timer = setTimeout(() => {
                        const error = new Error(
                            "Storage request timed out; a sent write may have persisted. Refresh and verify records before retrying",
                        );
                        reject(error);
                        expire(error);
                    }, requestTimeoutMs);
                }),
            ]);
        } finally {
            clearTimeout(timer);
        }
    }
    function request(value: StorageRequest) {
        const connection = active();
        return deadline(connection.request(value), (error) =>
            failConnection?.(error),
        );
    }
    let disposed = false;
    let failed = false;
    let failConnection: ((error: Error) => void) | undefined;
    let ready = false;
    let socket: StorageConnection | undefined;
    let persist: ((params: MutationFnParams<T>) => Promise<void>) | undefined;
    let queue = Promise.resolve();
    let reload: (() => Promise<{ receipt: true | Promise<void> }>) | undefined;

    function active() {
        if (disposed)
            throw new Error(
                "Storage collection is disposed; a sent write may still persist",
            );
        if (failed || !socket)
            throw new Error(
                "Storage connection unavailable; a sent write may have persisted. Refresh and verify records before retrying",
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
        active();
        let parent = "";
        for (const segment of directory.split("/")) {
            const listing = await request({
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
        const response = await request({
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
            const read = await request({
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
    const collection = createCollection<
        T,
        string,
        { refresh: () => Promise<void> }
    >({
        utils: {
            refresh: async (): Promise<void> => {
                if (disposed) throw new Error("Storage collection is disposed");
                if (collection.status === "idle") return collection.preload();
                const result = queue.then(() => {
                    if (disposed || !reload)
                        throw new Error("Storage collection is disposed");
                    return reload();
                });
                // Release the operation queue before waiting for visibility: a later
                // optimistic mutation can hold the receipt until its handler settles.
                queue = result.then(
                    () => undefined,
                    () => undefined,
                );
                const { receipt } = await result;
                await receipt;
                active();
            },
        },
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
                let controller = new AbortController();
                const detach = () => {
                    socket?.socket.removeEventListener("close", disconnected);
                    socket?.socket.removeEventListener("error", disconnected);
                };
                const fail = (error: Error) => {
                    if (disposed || failed) return;
                    failed = true;
                    ready = false;
                    detach();
                    socket?.close();
                    markError(error);
                };
                failConnection = fail;
                const disconnected = () =>
                    fail(
                        new Error(
                            "Storage connection closed; a sent write may have persisted. Refresh and verify records before retrying",
                        ),
                    );
                const publish = (records: T[]) => {
                    active();
                    begin();
                    truncate();
                    for (const value of records)
                        write({ type: "insert", value });
                    const receipt = commit();
                    if (receipt !== true)
                        void receipt.catch(() =>
                            fail(
                                new Error(
                                    "Storage reconciliation failed; Refresh and verify records before retrying",
                                ),
                            ),
                        );
                    return receipt;
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
                    active();
                    if (transaction.state !== "persisting")
                        throw new Error(
                            "Storage mutation was rolled back before sending",
                        );
                    let outcome: "acknowledged" | "rejected" | "ambiguous" =
                        "ambiguous";
                    try {
                        const response = await request(
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
                        publish(records);
                    } catch {
                        const error = new Error(
                            "Storage write outcome could not be reconciled; it may have persisted. Refresh and verify records before retrying",
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
                reload = async () => {
                    try {
                        if (disposed)
                            throw new Error("Storage collection is disposed");
                        if (failed || !socket) {
                            detach();
                            socket?.close();
                            socket = undefined;
                            failed = false;
                            controller = new AbortController();
                            const opened = await deadline(
                                client.openSocket(
                                    filesystemId,
                                    controller.signal,
                                ),
                                () => controller.abort(),
                            );
                            if (disposed) {
                                opened.close();
                                throw new Error(
                                    "Storage collection is disposed",
                                );
                            }
                            socket = opened;
                            socket.socket.addEventListener(
                                "close",
                                disconnected,
                            );
                            socket.socket.addEventListener(
                                "error",
                                disconnected,
                            );
                        }
                        const records = await snapshot();
                        const receipt = publish(records);
                        ready = true;
                        markReady();
                        return { receipt };
                    } catch (cause) {
                        const error =
                            cause instanceof Error
                                ? cause
                                : new Error("Storage snapshot failed");
                        fail(error);
                        throw error;
                    }
                };
                queue = reload()
                    .then(async ({ receipt }) => {
                        await receipt;
                    })
                    .catch(() => undefined);
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
