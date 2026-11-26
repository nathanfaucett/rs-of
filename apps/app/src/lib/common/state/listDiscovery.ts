export type ListOrder = "default" | "name-asc" | "name-desc";

const byName = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });

export function discoverRows<T extends { name?: string | null | undefined }>(
  rows: readonly T[],
  search: string,
  order: ListOrder,
  fields: (row: T) => readonly (string | null | undefined)[]
): readonly T[] {
  const query = search.trim().toLowerCase();
  const matches = query
    ? rows.filter((row) =>
        [row.name, ...fields(row)].some((value) => value?.toLowerCase().includes(query))
      )
    : rows;
  if (order === "default") return matches;
  const direction = order === "name-asc" ? 1 : -1;
  return [...matches].sort((a, b) => direction * byName.compare(a.name ?? "", b.name ?? ""));
}
