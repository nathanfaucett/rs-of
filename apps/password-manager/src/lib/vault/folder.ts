import * as v from "valibot";
import { SecretSchema, type Secret } from "../secrets/secret";

export const FolderSchema = v.strictObject({
    id: v.pipe(v.string(), v.uuid()),
    name: v.pipe(
        v.string(),
        v.check((name) => !!name.trim()),
    ),
});
export type Folder = v.InferOutput<typeof FolderSchema>;
export interface ItemSnapshot {
    items: Secret[];
    folders: Folder[];
}
export interface ItemMutation {
    upsert?: Secret[];
    deleteIds?: string[];
    folders?: Folder[];
    deleteFolderIds?: string[];
}

export function validateItemSnapshot(input: ItemSnapshot): ItemSnapshot {
    try {
        const items = v.parse(v.array(SecretSchema), input.items);
        const folders = v.parse(v.array(FolderSchema), input.folders);
        const folderIds = new Set(folders.map((folder) => folder.id));
        if (
            folderIds.size !== folders.length ||
            new Set(items.map((item) => item.id)).size !== items.length ||
            items.some(
                (item) =>
                    item.folderId != null && !folderIds.has(item.folderId),
            )
        )
            throw new Error("Invalid identity or membership");
        return { items, folders };
    } catch {
        throw new Error(
            "Malformed vault snapshot. No partial records or folders were loaded. Check the complete backup; do not clear browser data.",
        );
    }
}
