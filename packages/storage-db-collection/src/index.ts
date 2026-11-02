import type { StandardSchemaV1 } from "@standard-schema/spec";
import { createCollection, type MutationFnParams } from "@tanstack/db";
import type { ConnectBackend, StorageBackend } from "./adapters/backend.js";
export { createOfdbBackend, createWebSocketBackend } from "./adapters/index.js";
export type { ConnectBackend, StorageBackend } from "./adapters/backend.js";

export class UnacknowledgedStorageWriteError extends Error {
    constructor() {
        super(
            "Storage write was not acknowledged; authoritative records reloaded. Verify the outcome before retrying",
        );
        this.name = "UnacknowledgedStorageWriteError";
    }
}

export type StorageCollectionOptions<T extends object> = {
    connect: ConnectBackend;
    userId: string;
    applicationId: string;
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
    const { connect, userId, applicationId, directory, schema, getKey } =
        options;
    if (!userId || !applicationId)
        throw new Error(
            "Storage collection requires a User and Application ID",
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

    let disposed = false;
    let failed = false;
    let _failConnection: ((error: Error) => void) | undefined;
    let ready = false;
    let backend: StorageBackend | undefined;
    let persist: ((params: MutationFnParams<T>) => Promise<void>) | undefined;
    let queue: Promise<unknown> = Promise.resolve();
    let reload: (() => Promise<{ receipt: true | Promise<void> }>) | undefined;
    let disconnectHandler: (() => void) | undefined;

    function active(): StorageBackend {
        if (disposed)
            throw new Error(
                "Storage collection is disposed; a sent write may still persist",
            );
        if (failed || !backend)
            throw new Error(
                "Storage connection unavailable; a sent write may have persisted. Refresh and verify records before retrying",
            );
        return backend;
    }

    function path(key: string): string {
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
        const backend = active();
        const prefix = `${directory}/`;
        const entries = await backend.list(prefix);
        const records: T[] = [];
        for (const fullKey of entries) {
            if (!fullKey.endsWith(extension)) continue;
            const key = fullKey.slice(prefix.length, -extension.length);
            if (!fullKey.startsWith(prefix))
                throw new Error("Unsafe record key");
            // Validate key before reading (matches original behavior)
            path(key);
            const content = await backend.read(fullKey);
            if (content === undefined) continue;
            let input: unknown;
            try {
                input = deserialize(content);
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
                queue = result.then(
                    () => undefined,
                    () => undefined,
                );
                const { receipt } = await result;
                await receipt;
                active();
            },
        },
        id: JSON.stringify([userId, applicationId, directory, extension]),
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
                    // Backend-specific disconnect listeners are handled by the backend
                };
                const fail = (error: Error) => {
                    if (disposed || failed) return;
                    failed = true;
                    ready = false;
                    detach();
                    backend?.close?.();
                    markError(error);
                };
                _failConnection = fail;
                disconnectHandler = () =>
                    fail(
                        new Error(
                            "Storage connection closed; a sent write may have persisted. Refresh and verify records before retrying",
                        ),
                    );
                if (disconnectHandler)
                    backend?.onDisconnect?.(disconnectHandler);

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
                    const fullKey = path(key);
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
                        const b = active();
                        if (mutation.type === "delete") {
                            await b.delete(fullKey);
                        } else if (bytes !== undefined) {
                            await b.write(fullKey, bytes);
                        }
                        outcome = "acknowledged";
                    } catch (error) {
                        active();
                        if (
                            error instanceof Error &&
                            error.message.includes("not acknowledged")
                        ) {
                            outcome = "rejected";
                        }
                    }
                    try {
                        const records = await snapshot();
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

                let initialConnect = true;

                reload = async () => {
                    try {
                        if (disposed)
                            throw new Error("Storage collection is disposed");
                        if (initialConnect) {
                            initialConnect = false;
                            controller = new AbortController();
                            backend = await deadline(
                                connect(controller.signal),
                                () => controller.abort(),
                            );
                            if (disposed) {
                                backend?.close?.();
                                throw new Error(
                                    "Storage collection is disposed",
                                );
                            }
                            if (disconnectHandler)
                                backend.onDisconnect?.(disconnectHandler);
                        } else if (failed || !backend) {
                            detach();
                            backend?.close?.();
                            backend = undefined;
                            failed = false;
                            controller = new AbortController();
                            backend = await deadline(
                                connect(controller.signal),
                                () => controller.abort(),
                            );
                            if (disposed) {
                                backend?.close?.();
                                throw new Error(
                                    "Storage collection is disposed",
                                );
                            }
                            if (disconnectHandler)
                                backend.onDisconnect?.(disconnectHandler);
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

                queue = reload().catch(() => undefined);

                return () => {
                    disposed = true;
                    controller.abort();
                    detach();
                    backend?.close?.();
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
