import {
    DatabaseOperationError,
    type SqlRequest,
    type SqlResponse,
} from "@aicacia/storage-client";
import {
    type Collection,
    createCollection,
    type MutationFnParams,
} from "@tanstack/db";
import * as v from "valibot";
import { type Secret, SecretSchema } from "$lib/models/secret";
import {
    type SqlStorage,
    storageMessage,
    storageRequest,
} from "$lib/storage/sql";

export class UnacknowledgedStorageWriteError extends Error {
    constructor() {
        super(
            "Storage write was not acknowledged; authoritative records reloaded. Verify the outcome before retrying.",
        );
    }
}

export function createSecretCollection({
    client,
    databaseId,
}: {
    client: SqlStorage;
    databaseId: string;
}): Collection<
    Secret,
    string,
    { refresh: () => Promise<void> },
    typeof SecretSchema
> {
    const controller = new AbortController();
    let disposed = false;
    let ready = false;
    let queue = Promise.resolve();
    let persist: (params: MutationFnParams<Secret>) => Promise<void>;
    let reload: () => Promise<{ receipt: true | Promise<void> }>;
    function active() {
        if (disposed) throw new Error("Storage collection is disposed");
    }
    const execute = async (
        statements: SqlRequest["statements"],
        actions: ("read" | "write")[],
    ) => {
        active();
        const result = await storageRequest(
            (signal) =>
                client.executeSql(databaseId, { statements }, actions, signal),
            controller.signal,
        );
        active();
        return result;
    };
    async function snapshot(): Promise<Secret[]> {
        const response = await execute(
            [{ sql: "SELECT id, content FROM secrets", params: [] }],
            ["read"],
        );
        if (response.results.length !== 1)
            throw new Error("Malformed Storage snapshot");
        return parseSnapshot(response.results[0]);
    }
    function parseSnapshot(
        result: SqlResponse["results"][number] | undefined,
    ): Secret[] {
        if (
            result?.columns.join() !== "id,content" ||
            !Array.isArray(result.rows)
        )
            throw new Error("Malformed Storage snapshot");
        const seen = new Set<string>();
        return result.rows.map((row) => {
            try {
                if (
                    row.length !== 2 ||
                    row[0]?.type !== "uuid" ||
                    row[1]?.type !== "text"
                )
                    throw new Error();
                const record = v.parse(SecretSchema, JSON.parse(row[1].value));
                if (record.id !== row[0].value || seen.has(record.id))
                    throw new Error();
                seen.add(record.id);
                return record;
            } catch {
                throw new Error(
                    "Malformed Storage record. No partial records were loaded.",
                );
            }
        });
    }
    function mutate(params: MutationFnParams<Secret>) {
        const result = queue.then(async () => {
            active();
            if (!ready) throw new Error("Storage collection is not ready");
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
        typeof SecretSchema,
        string,
        { refresh: () => Promise<void> }
    >({
        id: databaseId,
        schema: SecretSchema,
        getKey: (record) => record.id,
        gcTime: Infinity,
        onInsert: mutate,
        onUpdate: mutate,
        onDelete: mutate,
        utils: {
            refresh: async () => {
                active();
                if (collection.status === "idle") return collection.preload();
                const result = queue.then(() => {
                    active();
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
        sync: {
            sync: ({
                begin,
                write,
                commit,
                truncate,
                markReady,
                markError,
            }) => {
                const fail = (error: Error) => {
                    if (disposed) return;
                    ready = false;
                    markError(error);
                };
                const publish = (records: Secret[]) => {
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
                                    "Storage reconciliation failed. Refresh and verify records before retrying.",
                                ),
                            ),
                        );
                    return receipt;
                };
                reload = async () => {
                    try {
                        const records = await snapshot();
                        const receipt = publish(records);
                        ready = true;
                        markReady();
                        return { receipt };
                    } catch (cause) {
                        const error = new Error(
                            cause instanceof Error &&
                                cause.message.startsWith("Malformed Storage")
                                ? cause.message
                                : storageMessage(cause),
                        );
                        fail(error);
                        throw error;
                    }
                };
                persist = async ({ transaction }) => {
                    const [mutation] = transaction.mutations;
                    if (!mutation || transaction.mutations.length !== 1)
                        throw new Error(
                            "Only single-secret mutations are supported",
                        );
                    const key = String(mutation.key);
                    if (!v.safeParse(SecretSchema.entries.id, key).success)
                        throw new Error("Invalid secret identity");
                    const value =
                        mutation.type === "delete"
                            ? null
                            : v.parse(SecretSchema, mutation.modified);
                    if (value && value.id !== key)
                        throw new Error("Secret identity mismatch");
                    active();
                    if (transaction.state !== "persisting")
                        throw new Error("Mutation retired before sending");
                    let outcome: "acknowledged" | "rejected" | "uncertain" =
                        "uncertain";
                    let acknowledgedRecords: Secret[] | undefined;
                    try {
                        const response = await execute(
                            [
                                {
                                    sql:
                                        mutation.type === "delete"
                                            ? "DELETE FROM secrets WHERE id = ?"
                                            : mutation.type === "insert"
                                              ? "INSERT INTO secrets (id, content) VALUES (?, ?)"
                                              : "UPDATE secrets SET content = ? WHERE id = ?",
                                    params: value
                                        ? mutation.type === "insert"
                                            ? [
                                                  { type: "uuid", value: key },
                                                  {
                                                      type: "text",
                                                      value: JSON.stringify(
                                                          value,
                                                      ),
                                                  },
                                              ]
                                            : [
                                                  {
                                                      type: "text",
                                                      value: JSON.stringify(
                                                          value,
                                                      ),
                                                  },
                                                  { type: "uuid", value: key },
                                              ]
                                        : [{ type: "uuid", value: key }],
                                },
                                {
                                    sql: "SELECT id, content FROM secrets",
                                    params: [],
                                },
                            ],
                            ["read", "write"],
                        );
                        if (
                            response.results.length === 2 &&
                            Array.isArray(response.results[0]?.rows) &&
                            response.results[0].rows.length === 0
                        ) {
                            acknowledgedRecords = parseSnapshot(
                                response.results[1],
                            );
                            outcome = "acknowledged";
                        }
                    } catch (cause) {
                        active();
                        // Authorization can fail after commit; only pre-commit errors establish rejection.
                        if (
                            cause instanceof DatabaseOperationError &&
                            [400, 413, 422].includes(cause.status)
                        )
                            outcome = "rejected";
                    }
                    try {
                        const records =
                            acknowledgedRecords ?? (await snapshot());
                        // The core holds sync visibility until this mutation settles; awaiting its receipt here deadlocks.
                        publish(records);
                        if (outcome === "acknowledged") {
                            const stored = records.find(
                                (record) => record.id === key,
                            );
                            if (
                                mutation.type === "delete"
                                    ? !!stored
                                    : !stored ||
                                      JSON.stringify(stored) !==
                                          JSON.stringify(value)
                            )
                                outcome = "uncertain";
                        }
                    } catch {
                        const error = new Error(
                            "Storage write outcome could not be reconciled; it may have persisted. Refresh and verify records before retrying.",
                        );
                        fail(error);
                        throw error;
                    }
                    if (outcome === "uncertain")
                        throw new UnacknowledgedStorageWriteError();
                    if (outcome === "rejected")
                        throw new Error(
                            "Storage write rejected; authoritative records reloaded. Review before retrying.",
                        );
                };
                queue = reload()
                    .then(async ({ receipt }) => {
                        await receipt;
                    })
                    .catch(() => undefined);
                return () => {
                    disposed = true;
                    ready = false;
                    controller.abort();
                };
            },
        },
    });
    return collection;
}
