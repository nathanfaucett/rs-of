import type { Secret } from "$lib/secrets/secret";
import { type ItemSnapshot, StaleItemMutationError } from "$lib/vault/folder";
import type { VaultHandle } from "$lib/vault/vault";
import { normalizeTimestamp } from "$lib/timestamp";
import { parsePersonalJson, type ParsedImport } from "./personal-json";

export type ImportPreview = Awaited<
    ReturnType<typeof preparePersonalJsonImport>
>;

function logical(
    item: Secret,
    folders: Map<string, string>,
    omitCreated: boolean,
    omitUpdated: boolean,
): string {
    const {
        id: _id,
        importSource: _source,
        folderId,
        createdAt,
        updatedAt,
        ...content
    } = item;
    const ordered = (value: unknown): unknown => {
        if (Array.isArray(value)) return value.map(ordered);
        if (value && typeof value === "object")
            return Object.fromEntries(
                Object.entries(value)
                    .filter(([, entry]) => entry != null)
                    .sort(([a], [b]) => a.localeCompare(b))
                    .map(([key, entry]) => [
                        key,
                        key === "changedAt" && typeof entry === "string"
                            ? normalizeTimestamp(entry)
                            : ordered(entry),
                    ]),
            );
        return value;
    };
    // Absent optional scalars export as null; array order remains significant.
    return JSON.stringify(
        ordered({
            ...content,
            folder: folderId ? folders.get(folderId) : null,
            ...(omitCreated
                ? {}
                : { createdAt: normalizeTimestamp(createdAt) }),
            ...(omitUpdated
                ? {}
                : { updatedAt: normalizeTimestamp(updatedAt) }),
        }),
        (_key, value) => (value === undefined ? null : value),
    );
}
function duplicates(parsed: ParsedImport, snapshot: ItemSnapshot) {
    let sourceIds = 0;
    let logicalCount = 0;
    const sources = new Set(
        snapshot.items
            .map((item) => item.importSource?.id)
            .filter((id) => id !== undefined),
    );
    const importedFolders = new Map(
        parsed.folders.map((folder) => [folder.id, folder.name]),
    );
    const existingFolders = new Map(
        snapshot.folders.map((folder) => [folder.id, folder.name]),
    );
    const indexes = new Map<string, Set<string>>();
    for (const item of parsed.items) {
        if (item.importSource && sources.has(item.importSource.id)) sourceIds++;
        const omitCreated = parsed.missingDates.has(`${item.id}/createdAt`);
        const omitUpdated = parsed.missingDates.has(`${item.id}/updatedAt`);
        const key = `${omitCreated}/${omitUpdated}`;
        let index = indexes.get(key);
        if (!index) {
            index = new Set(
                snapshot.items.map((existing) =>
                    logical(
                        existing,
                        existingFolders,
                        omitCreated,
                        omitUpdated,
                    ),
                ),
            );
            indexes.set(key, index);
        }
        if (index.has(logical(item, importedFolders, omitCreated, omitUpdated)))
            logicalCount++;
    }
    return { sourceIds, logical: logicalCount };
}

export async function preparePersonalJsonImport(
    vault: VaultHandle,
    text: string,
) {
    let parsed: ParsedImport | null = parsePersonalJson(text);
    let retired = false;
    let pending = false;
    let attempted = false;
    const snapshot = parsed.issues.length ? null : await vault.readItems();
    const matches = snapshot
        ? duplicates(parsed, snapshot)
        : { sourceIds: 0, logical: 0 };
    const counts = {
        login: 0,
        secureNote: 0,
        card: 0,
        identity: 0,
        sshKey: 0,
        folders: parsed.folders.length,
    };
    for (const item of parsed.items) counts[item.kind]++;
    const issues = parsed.issues;
    const warnings = parsed.warnings;
    return {
        counts,
        issues,
        warnings,
        duplicates: matches,
        retire() {
            retired = true;
            parsed = null;
        },
        async commit(choice?: "import-as-new") {
            if (retired || !parsed)
                throw new Error(
                    "Import preview is closed. Validate the file again.",
                );
            if (pending || attempted)
                throw new Error(
                    "Import already submitted. Reopen and verify before retrying.",
                );
            if (issues.length || !snapshot)
                throw new Error(
                    "Import blocked. Correct every validation issue first.",
                );
            if (
                (matches.sourceIds || matches.logical) &&
                choice !== "import-as-new"
            )
                throw new Error(
                    "Duplicates found. Cancel or explicitly choose import-as-new for the whole batch.",
                );
            pending = true;
            try {
                // Serialize import confirmations across tabs; the vault owns atomic publication.
                await navigator.locks.request(
                    `password-manager-import-${vault.id}`,
                    async () => {
                        if (retired || !parsed)
                            throw new Error("Import preview is closed.");
                        const current = await vault.readItems();
                        if (retired || !parsed)
                            throw new Error("Import preview is closed.");
                        if (
                            JSON.stringify(current) !== JSON.stringify(snapshot)
                        )
                            throw new Error(
                                "Vault changed since preview. Cancel and validate the file again.",
                            );
                        const batch = parsed;
                        attempted = true;
                        await vault.mutateItems({
                            upsert: batch.items,
                            folders: batch.folders,
                            expectedSnapshot: snapshot,
                        });
                        if (retired)
                            throw new Error(
                                "Import session closed. Reopen and verify before retrying.",
                            );
                        parsed = null;
                        retired = true;
                    },
                );
            } catch (cause) {
                if (cause instanceof StaleItemMutationError) {
                    parsed = null;
                    retired = true;
                    throw new Error(
                        "Import not submitted. Vault changed since preview. Cancel and validate the file again.",
                    );
                }
                if (attempted) {
                    parsed = null;
                    retired = true;
                    throw new Error(
                        "Import could not be confirmed. The vault contains either the old or the complete new batch. Lock, reopen, and verify before retrying. Do not clear browser storage.",
                    );
                }
                throw new Error(
                    "Import not submitted. The vault or session changed. Cancel and validate the file again.",
                );
            } finally {
                pending = false;
            }
        },
    };
}
