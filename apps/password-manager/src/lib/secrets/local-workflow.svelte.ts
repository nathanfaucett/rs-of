import { useLiveQuery } from "@tanstack/svelte-db";
import { SvelteDate } from "svelte/reactivity";
import * as v from "valibot";
import type { VaultHandle } from "$lib/vault/vault";
import type { Folder } from "$lib/vault/folder";
import {
    createLocalSecretCollection,
    UnacknowledgedStorageWriteError,
} from "./local-collection";
import { type Login, type Secret, LoginSchema } from "./secret";

import {
    constructSecret,
    createSecretDraft,
    type SecretDraft,
} from "./secret-draft";

export function createLocalSecretWorkflow({ vault }: { vault: VaultHandle }) {
    let folders = $state.raw<Folder[]>([]);
    let folderFilter = $state("all");
    const collection = createLocalSecretCollection({
        vault,
        onFolders: (value) => {
            if (active) folders = value;
        },
    });
    let editor = $state<{
        record: Login | null;
        draft: SecretDraft;
        confirmPassword: string;
        showPassword: boolean;
    } | null>(null);
    let selectedId = $state<string | null>(null);

    const dirty = $derived(
        !!editor &&
            (JSON.stringify(editor.draft) !==
                JSON.stringify(createSecretDraft(editor.record)) ||
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
    function edit(value: Secret | null = null) {
        if (!canEdit()) return;
        if (value && value.kind !== "login") return;
        const record = value
            ? v.parse(v.object(LoginSchema.entries), value)
            : null;
        selectedId = record?.id ?? null;
        editor = {
            record,
            draft: createSecretDraft(record),
            confirmPassword: "",
            showPassword: false,
        };
        error = null;
        notice = "";
    }
    function canEdit() {
        return active && !busy && !loadError && query.isReady && !query.isError;
    }
    async function folderAction(
        action: () => Promise<unknown>,
        success: string,
    ) {
        if (!canEdit()) return;
        busy = true;
        editor = null;
        error = null;
        notice = "";
        try {
            await action();
            if (!active) return;
            await collection.utils.refresh();
            if (!active) return;
            if (
                folderFilter !== "all" &&
                folderFilter !== "unfiled" &&
                !folders.some((folder) => folder.id === folderFilter)
            )
                folderFilter = "unfiled";
            notice = success;
        } catch (cause) {
            if (!active) return;
            error = message(cause);
            try {
                await collection.utils.refresh();
            } catch {
                if (active)
                    loadError =
                        "Vault read failed. Reopen and verify before retrying.";
            }
        } finally {
            if (active) busy = false;
        }
    }
    return {
        get folders() {
            return active && !loadError
                ? [...folders].sort(
                      (a, b) =>
                          a.name.localeCompare(b.name) ||
                          a.id.localeCompare(b.id),
                  )
                : [];
        },
        get folderFilter() {
            return folderFilter;
        },
        setFolderFilter(value: string) {
            if (!canEdit()) return;
            folderFilter = value;
            selectedId = null;
            editor = null;
        },
        createFolder(name: string) {
            return folderAction(
                () => vault.createFolder(name),
                "Folder created.",
            );
        },
        renameFolder(id: string, name: string) {
            return folderAction(
                () => vault.renameFolder(id, name),
                "Folder renamed.",
            );
        },
        deleteFolder(id: string) {
            return folderAction(
                () => vault.deleteFolder(id),
                "Folder deleted. Items are unfiled.",
            );
        },
        moveItem(id: string, folderId: string | null) {
            return folderAction(
                () => vault.moveItems([id], folderId),
                "Item moved.",
            );
        },
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
                ? query.data.filter(
                      (record) =>
                          folderFilter === "all" ||
                          (folderFilter === "unfiled"
                              ? record.folderId == null
                              : record.folderId === folderFilter),
                  )
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
            if (
                draft.secret &&
                draft.secret !== previous?.secret &&
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
                error = "Invalid Login: A name is required; check the fields.";
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
            folders = [];
            folderFilter = "all";
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
