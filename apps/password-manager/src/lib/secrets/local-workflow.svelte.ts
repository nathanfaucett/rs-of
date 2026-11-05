import { useLiveQuery } from "@tanstack/svelte-db";
import { SvelteDate } from "svelte/reactivity";
import * as v from "valibot";
import type { VaultHandle } from "$lib/vault/vault";
import {
    createLocalSecretCollection,
    UnacknowledgedStorageWriteError,
} from "./local-collection";
import { type Secret, LoginSchema, SecureNoteSchema } from "./secret";

import {
    constructSecret,
    createSecretDraft,
    type SecretDraft,
    type EditableSecret,
    type EditableKind,
} from "./secret-draft";

export function createLocalSecretWorkflow({ vault }: { vault: VaultHandle }) {
    const collection = createLocalSecretCollection({ vault });
    let editor = $state<{
        record: EditableSecret | null;
        draft: SecretDraft;
        confirmPassword: string;
        showPassword: boolean;
    } | null>(null);
    let selectedId = $state<string | null>(null);

    const dirty = $derived(
        !!editor &&
            (JSON.stringify(editor.draft) !==
                JSON.stringify(createSecretDraft(editor.record, editor.draft.kind)) ||
                editor.confirmPassword !== ""),
    );
    let loadError = $state<string | null>(null);
    let error = $state<string | null>(null);
    let notice = $state("");
    let busy = $state(false);
    let refreshing = $state(false);
    let active = $state(true);
    let query: ReturnType<typeof liveQuery>;
    const detail = $derived(
        active && !loadError && query.isReady && !query.isError
            ? (query.data.find((record) => record.id === selectedId) ?? null)
            : null,
    );
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
            selectedId = null;
            notice = "";
            loadError =
                "Local vault read failed. Refresh and verify records before retrying.";
        }
    });
    void collection.preload().catch((cause: unknown) => {
        if (active) loadError = message(cause);
    });
    function edit(value: Secret | null = null, kind: EditableKind = "login") {
        if (!canEdit()) return;
        if (value && value.kind !== "login" && value.kind !== "secureNote") return;
        const record = value
            ? value.kind === "login"
                ? v.parse(v.object(LoginSchema.entries), value)
                : v.parse(v.object(SecureNoteSchema.entries), value)
            : null;
        selectedId = record?.id ?? null;
        editor = {
            record,
            draft: createSecretDraft(record, kind),
            confirmPassword: "",
            showPassword: false,
        };
        error = null;
        notice = "";
    }
    function canEdit() {
        return active && !busy && !loadError && query.isReady && !query.isError;
    }
    return {
        get detail() {
            return detail;
        },
        get dirty() {
            return dirty;
        },
        open(id: string | null) {
            if (!canEdit()) return;
            editor = null;
            selectedId = id;
            error = null;
            notice = "";
        },
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
                    ? "Local vault read failed. Refresh and verify records before retrying."
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
            if (!canEdit() || !editor || draft.kind !== editor.draft.kind) return;
            error = null;
            notice = "";
            const previous = editor.record;
            if (
                draft.kind === "login" &&
                draft.secret &&
                draft.secret !== (previous?.kind === "login" ? previous.secret : undefined) &&
                (!editor.confirmPassword ||
                    draft.secret !== editor.confirmPassword)
            ) {
                error =
                    "Passwords do not match. Confirm the password before saving.";
                return;
            }
            const result = constructSecret(
                previous,
                draft,
                previous?.id ?? newSecretId(),
                new SvelteDate().toISOString(),
            );
            if (!result.success) {
                error = draft.kind === "login"
                    ? "Invalid Login: A name is required; check the fields."
                    : "Invalid Secure Note: A name is required; check the fields.";
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
                editor = null;
                selectedId = value.id;
                notice = "Saved.";
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
            editor = null;
            selectedId = null;
            busy = true;
            refreshing = true;
            error = null;
            notice = "";
            try {
                await collection.utils.refresh();
                if (!active) return;
                loadError = null;
                notice = "Refreshed.";
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
                notice = "Deleted.";
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
            selectedId = null;
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
    return cause instanceof Error
        ? cause.message
        : "Local vault operation failed";
}

function newSecretId(): string {
    const random = crypto.randomUUID();
    const timestamp = Date.now().toString(16).padStart(12, "0");
    return `${timestamp.slice(0, 8)}-${timestamp.slice(8)}-7${random.slice(15)}`;
}
