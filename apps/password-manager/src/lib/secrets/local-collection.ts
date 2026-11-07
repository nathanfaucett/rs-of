import {
    type Collection,
    createCollection,
    type MutationFnParams,
} from "@tanstack/db";
import * as v from "valibot";
import type { VaultHandle } from "$lib/vault/vault";
import { type Folder, StaleItemMutationError } from "$lib/vault/folder";
import { type Secret, SecretSchema, SecretIdSchema } from "./secret";

export class UnacknowledgedStorageWriteError extends Error {
    constructor() {
        super(
            "Storage write was not acknowledged; authoritative records reloaded. Verify the outcome before retrying.",
        );
        this.name = "UnacknowledgedStorageWriteError";
    }
}

export function createLocalSecretCollection({
    vault,
    onFolders = () => {},
}: {
    vault: VaultHandle;
    onFolders?: (folders: Folder[]) => void;
}): Collection<
    Secret,
    string,
    { refresh: () => Promise<void> },
    typeof SecretSchema
> {
    let disposed = false;
    let ready = false;
    let queue = Promise.resolve();
    let persist: (params: MutationFnParams<Secret>) => Promise<void>;
    let reload: () => Promise<{ receipt: true | Promise<void> }>;

    function active() {
        if (disposed) throw new Error("Collection is disposed");
    }

    async function snapshot(): Promise<Secret[]> {
        active();
        const snapshot = await vault.readItems();
        active();
        onFolders(snapshot.folders);
        return snapshot.items;
    }

    function mutate(params: MutationFnParams<Secret>) {
        const result = queue.then(async () => {
            active();
            if (!ready) throw new Error("Collection is not ready");
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
        id: `local-vault-${vault.id}`,
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
                                    "Reconciliation failed. Refresh and verify records before retrying.",
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
                        const error =
                            cause instanceof Error
                                ? cause
                                : new Error("Snapshot failed");
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
                    if (!v.safeParse(SecretIdSchema, key).success)
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

                    try {
                        if (mutation.type === "delete") {
                            await vault.deleteSecret(key);
                        } else if (value) {
                            await vault.encryptAndStore(JSON.stringify(value));
                        }
                    } catch (cause) {
                        active();
                        if (cause instanceof StaleItemMutationError) {
                            await reload();
                            throw cause;
                        }
                        const error = new Error(
                            cause instanceof Error
                                ? cause.message
                                : "Local write failed",
                        );
                        fail(error);
                        throw error;
                    }

                    let records: Secret[];
                    try {
                        records = await snapshot();
                        publish(records);
                    } catch {
                        const error = new Error(
                            "Write outcome could not be reconciled; it may have persisted. Refresh and verify records before retrying.",
                        );
                        fail(error);
                        throw error;
                    }
                    const stored = records.find((record) => record.id === key);
                    if (
                        value
                            ? JSON.stringify(stored) !== JSON.stringify(value)
                            : stored !== undefined
                    )
                        throw new UnacknowledgedStorageWriteError();
                };
                queue = reload()
                    .then(async ({ receipt }) => {
                        await receipt;
                    })
                    .catch(() => undefined);
                return () => {
                    disposed = true;
                    ready = false;
                };
            },
        },
    });
    collection.on("status:change", ({ status }) => {
        if (status === "cleaned-up") disposed = true;
    });
    return collection;
}
