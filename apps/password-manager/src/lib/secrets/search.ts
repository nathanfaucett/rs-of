import type { Secret } from "./secret";

export type ItemTypeFilter = "all" | Secret["kind"];
export type ItemSort =
    | "favorites"
    | "nameAsc"
    | "nameDesc"
    | "updatedDesc"
    | "createdDesc";
export type ItemSearch = {
    text: string;
    kind: ItemTypeFilter;
    favoritesOnly: boolean;
    sort: ItemSort;
};

function normalize(value: string): string {
    return value.normalize("NFC").toLowerCase();
}

function compare(a: string, b: string): number {
    return a < b ? -1 : a > b ? 1 : 0;
}

export function searchItems(
    records: readonly Secret[],
    search: ItemSearch,
): Secret[] {
    const text = normalize(search.text.trim());
    return records
        .filter((record) => {
            if (
                record.deletedAt ||
                (search.kind !== "all" && record.kind !== search.kind) ||
                (search.favoritesOnly && !record.favorite)
            )
                return false;
            if (!text) return true;
            const values = [
                record.name,
                record.notes,
                ...record.fields.map((field) => field.name),
            ];
            if (record.kind === "login" || record.kind === "identity")
                values.push(record.username);
            if (record.kind === "login")
                values.push(...record.uris.map((uri) => uri.uri));
            return values.some(
                (value) => value != null && normalize(value).includes(text),
            );
        })
        .sort((a, b) => {
            const name = compare(normalize(a.name), normalize(b.name));
            let order = name;
            switch (search.sort) {
                case "favorites":
                    order = Number(b.favorite) - Number(a.favorite) || name;
                    break;
                case "nameDesc":
                    order = -name;
                    break;
                case "updatedDesc":
                    order =
                        Date.parse(b.updatedAt) - Date.parse(a.updatedAt) ||
                        name;
                    break;
                case "createdDesc":
                    order =
                        Date.parse(b.createdAt) - Date.parse(a.createdAt) ||
                        name;
                    break;
            }
            return order || compare(a.id, b.id);
        });
}
