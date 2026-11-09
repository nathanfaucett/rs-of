import { useLiveQuery } from "@tanstack/svelte-db";
import { SvelteDate } from "svelte/reactivity";
import * as v from "valibot";
import { type Folder, StaleItemMutationError } from "../folder";
import type { VaultHandleLike } from "$lib/vault/types";
import {
    createLocalSecretCollection,
    UnacknowledgedStorageWriteError,
} from "./collection";
import {
    CardSchema,
    IdentitySchema,
    LoginSchema,
    type Secret,
    SecureNoteSchema,
    SshKeySchema,
} from "./secret";

import {
    constructSecret,
    createSecretDraft,
    type EditableKind,
    type EditableSecret,
    type SecretDraft,
} from "./secret-draft";

export function createLocalSecretWorkflow({ vault }: { vault: VaultHandleLike }) {
    let folders = $state.raw<Folder[]>([]);
    let folderFilter = $state("all");
    const collection = createLocalSecretCollection({
        vault,
        onFolders: (value) => {
            if (active) folders = value;
        },
    });
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
                JSON.stringify(
                    createSecretDraft(editor.record, editor.draft.kind),
                ) ||
                editor.confirmPassword !== ""),
    );
    let loadError = $state<string | null>(null);
    let error = $state<string | null>(null);
    let notice = $state("");
    let busy = $state(false);
    let refreshing = $state(false);
    let active = $state(true);
    let trash = $state(false);
    let query: ReturnType<typeof liveQuery>;
    const detail = $derived(
        active && !loadError && query.isReady && !query.isError
            ? (query.data.find(
                  (record) => record.id === selectedId && !record.deletedAt,
              ) ?? null)
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
        if (value?.deletedAt) return;
        const record = value
            ? value.kind === "login"
                ? v.parse(v.object(LoginSchema.entries), value)
                : value.kind === "card"
                  ? v.parse(v.object(CardSchema.entries), value)
                  : value.kind === "identity"
                    ? v.parse(v.object(IdentitySchema.entries), value)
                    : value.kind === "sshKey"
                      ? v.parse(v.object(SshKeySchema.entries), value)
                      : v.parse(v.object(SecureNoteSchema.entries), value)
            : null;
        selectedId = record?.id ?? null;
        const draft = createSecretDraft(record, kind);
        // Pre-fill confirm password when editing an existing login
        const confirmPassword = record?.kind === "login" ? record.secret : "";
        editor = {
            record,
            draft,
            confirmPassword,
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
    async function changeTrash(
        value: Secret,
        action: "trash" | "restore" | "purge",
    ) {
        if (!canEdit()) return;
        const current = collection.get(value.id);
        if (
            !current ||
            (action === "trash" ? !!current.deletedAt : !current.deletedAt)
        )
            return;
        error = null;
        notice = "";
        busy = true;
        editor = null;
        selectedId = null;
        try {
            const tx =
                action === "purge"
                    ? collection.delete(current.id)
                    : collection.update(current.id, (record) => {
                          if (action === "trash")
                              record.deletedAt = new SvelteDate().toISOString();
                          else {
                              delete record.deletedAt;
                              if (
                                  record.folderId != null &&
                                  !folders.some(
                                      (folder) => folder.id === record.folderId,
                                  )
                              )
                                  record.folderId = null;
                          }
                      });
            await tx.isPersisted.promise;
            if (!active) return;
            notice =
                action === "trash"
                    ? "Moved to Trash."
                    : action === "restore"
                      ? "Restored."
                      : "Permanently deleted.";
        } catch (cause) {
            if (active) error = message(cause);
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
                          !record.deletedAt &&
                          (folderFilter === "all" ||
                              (folderFilter === "unfiled"
                                  ? record.folderId == null
                                  : record.folderId === folderFilter)),
                  )
                : [];
        },
        get trashRecords() {
            return active && !loadError && query.isReady && !query.isError
                ? query.data.filter((record) => !!record.deletedAt)
                : [];
        },
        get trash() {
            return trash;
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
            if (!canEdit() || !editor || draft.kind !== editor.draft.kind)
                return false;
            error = null;
            notice = "";
            const previous = editor.record;
            if (
                draft.kind === "login" &&
                draft.secret &&
                draft.secret !==
                    (previous?.kind === "login"
                        ? previous.secret
                        : undefined) &&
                (!editor.confirmPassword ||
                    draft.secret !== editor.confirmPassword)
            ) {
                error =
                    "Passwords do not match. Confirm the password before saving.";
                return false;
            }
            const result = constructSecret(
                previous,
                draft,
                previous?.id ?? newSecretId(),
                new SvelteDate().toISOString(),
            );
            if (!result.success) {
                const fieldIssue = result.issues.find((issue) =>
                    issue.message.startsWith("Custom field "),
                );
                if (fieldIssue) {
                    error = fieldIssue.message;
                    return false;
                }
                error =
                    draft.kind === "login"
                        ? "Invalid Login: A name is required; check the fields."
                        : draft.kind === "card"
                          ? "Invalid Card: A name is required; check the fields."
                          : draft.kind === "identity"
                            ? "Invalid Identity: A name is required; check the fields."
                            : draft.kind === "sshKey"
                              ? "Invalid SSH Key: A name and nonblank private key are required; check the fields."
                              : "Invalid Secure Note: A name is required; check the fields.";
                return false;
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
                if (!active) return false;
                busy = false;
                editor = null;
                selectedId = value.id;
                notice = "Saved.";
                return true;
            } catch (cause) {
                if (active) {
                    error = message(cause);
                    if (
                        cause instanceof UnacknowledgedStorageWriteError ||
                        cause instanceof StaleItemMutationError
                    )
                        editor = null;
                }
                return false;
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
        setTrashMode(value: boolean) {
            if (!canEdit()) return;
            trash = value;
            selectedId = null;
            editor = null;
        },
        delete: (value: Secret) => changeTrash(value, "trash"),
        restore: (value: Secret) => changeTrash(value, "restore"),
        deletePermanently: (value: Secret) => changeTrash(value, "purge"),
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
