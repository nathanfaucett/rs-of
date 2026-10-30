import { useLiveQuery } from "@tanstack/svelte-db";
import { SvelteDate } from "svelte/reactivity";
import * as v from "valibot";
import {
    createSecretCollection,
    UnacknowledgedStorageWriteError,
} from "$lib/collections/secrets/collection";
import type { SqlStorage } from "$lib/storage/sql";
import { type Secret, SecretSchema } from "$lib/models/secret";

export type SecretDraft = {
    name: string;
    secret: string;
    uri: string;
    notes: string;
    favorite: boolean;
};

function constructSecret(
    previous: Secret | null,
    draft: SecretDraft,
    id: string,
    now: string,
) {
    const uris = previous?.uris.slice() ?? [];
    if (draft.uri)
        uris.splice(0, uris.length ? 1 : 0, {
            uri: draft.uri,
            match: uris[0]?.match ?? "domain",
        });
    else if (uris.length) uris.shift();
    return v.safeParse(SecretSchema, {
        ...previous,
        id,
        name: draft.name,
        secret: draft.secret,
        uris,
        notes:
            draft.notes === (previous?.notes ?? "")
                ? previous?.notes
                : draft.notes || undefined,
        favorite: draft.favorite,
        fields: previous?.fields ?? [],
        history:
            previous && previous.secret !== draft.secret
                ? [
                      ...previous.history,
                      { secret: previous.secret, changedAt: now },
                  ]
                : (previous?.history ?? []),
        createdAt: previous?.createdAt ?? now,
        updatedAt: now,
    });
}

export function createSecretWorkflow(options: {
    client: SqlStorage;
    databaseId: string;
}) {
    const collection = createSecretCollection(options);
    let editor = $state.raw<{ record: Secret | null } | null>(null);
    let loadError = $state<string | null>(null);
    let error = $state<string | null>(null);
    let notice = $state("");
    let busy = $state(false);
    let refreshing = $state(false);
    let active = $state(true);
    let query: ReturnType<typeof liveQuery>;
    function liveQuery() {
        return useLiveQuery((q) =>
            q
                .from({ secret: collection })
                .orderBy(({ secret }) => secret.favorite, "desc")
                .orderBy(({ secret }) => secret.name, "asc"),
        );
    }
    const stop = $effect.root(() => {
        query = liveQuery();
    });
    const unsubscribe = collection.on("status:change", ({ status }) => {
        if (active && status === "error") {
            editor = null;
            notice = "";
            loadError =
                "Storage connection failed; a sent write may have persisted. Refresh and verify records before retrying.";
        }
    });
    void collection.preload().catch((cause: unknown) => {
        if (active) loadError = message(cause);
    });
    function edit(value: Secret | null = null) {
        if (!canEdit()) return;
        editor = { record: value ? v.parse(SecretSchema, value) : null };
        error = null;
        notice = "";
    }
    function canEdit() {
        return active && !busy && !loadError && query.isReady && !query.isError;
    }
    return {
        get busy() {
            return busy;
        },
        get refreshing() {
            return refreshing;
        },
        get loadError() {
            if (!active) return null;
            return (
                loadError ??
                (query.isError
                    ? "Storage connection failed. Refresh and verify records before retrying."
                    : null)
            );
        },
        get canEdit() {
            return canEdit();
        },
        get canRefresh() {
            return (
                active &&
                !busy &&
                (query.isReady || query.isError || !!loadError)
            );
        },
        get loading() {
            return active && !loadError && !query.isReady && !query.isError;
        },
        get records() {
            return active && !loadError && query.isReady && !query.isError
                ? query.data
                : [];
        },
        get editor() {
            return editor;
        },
        get error() {
            return error;
        },
        get notice() {
            return notice;
        },
        edit,
        cancel() {
            if (!active || busy) return;
            editor = null;
            error = null;
            notice = "";
        },
        async save(draft: SecretDraft) {
            if (!canEdit() || !editor) return;
            error = null;
            notice = "";
            const previous = editor.record;
            const result = constructSecret(
                previous,
                draft,
                previous?.id ?? newSecretId(),
                new SvelteDate().toISOString(),
            );
            if (!result.success) {
                error =
                    "Invalid secret: Name and Secret are required; check the fields.";
                return;
            }
            busy = true;
            try {
                const value = result.output;
                const tx = previous
                    ? collection.update(value.id, (record) => {
                          Object.assign(record, value);
                      })
                    : collection.insert(value);
                await tx.isPersisted.promise;
                if (!active) return;
                busy = false;
                const authoritative = collection.get(value.id);
                if (authoritative) edit(authoritative);
                else editor = null;
                notice = "Saved. Authoritative records reloaded.";
            } catch (cause) {
                if (active) {
                    error = message(cause);
                    if (cause instanceof UnacknowledgedStorageWriteError)
                        editor = null;
                }
            } finally {
                if (active) busy = false;
            }
        },
        async refresh() {
            if (
                !active ||
                busy ||
                (!query.isReady && !query.isError && !loadError)
            )
                return;
            busy = true;
            refreshing = true;
            error = null;
            notice = "";
            try {
                await collection.utils.refresh();
                if (!active) return;
                editor = null;
                loadError = null;
                notice = "Refreshed. Authoritative records reloaded.";
            } catch (cause) {
                if (active) loadError = message(cause);
            } finally {
                if (active) {
                    busy = false;
                    refreshing = false;
                }
            }
        },
        async delete(value: Secret) {
            if (!canEdit()) return;
            error = null;
            notice = "";
            busy = true;
            try {
                await collection.delete(value.id).isPersisted.promise;
                if (!active) return;
                if (editor?.record?.id === value.id) editor = null;
                notice = "Deleted. Authoritative records reloaded.";
            } catch (cause) {
                if (active) {
                    error = message(cause);
                    if (
                        cause instanceof UnacknowledgedStorageWriteError &&
                        editor?.record?.id === value.id
                    )
                        editor = null;
                }
            } finally {
                if (active) busy = false;
            }
        },
        dispose() {
            if (!active) return;
            active = false;
            busy = false;
            refreshing = false;
            loadError = null;
            editor = null;
            error = null;
            notice = "";
            unsubscribe();
            const queryCollection = query.collection;
            stop();
            void queryCollection?.cleanup();
            void collection.cleanup();
        },
    };
}

function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : "Storage request failed";
}

function newSecretId(): string {
    const random = crypto.randomUUID();
    const timestamp = Date.now().toString(16).padStart(12, "0");
    return `${timestamp.slice(0, 8)}-${timestamp.slice(8)}-7${random.slice(15)}`;
}
