import type { Database, DbValue } from "@ofsdk/sql";
import type { QueryResult } from "./core";

function encode(value: unknown): DbValue {
  if (value === null) return { type: "null" };
  if (typeof value === "string") return { type: "text", value };
  if (typeof value === "boolean") return { type: "bool", value };
  if (typeof value === "bigint" || (typeof value === "number" && Number.isSafeInteger(value))) {
    return { type: "integer", value: String(value) };
  }
  throw new TypeError("SQL parameters must be text, booleans, integers or null");
}

function decode(value: DbValue): unknown {
  if (value.type === "null") return null;
  if (value.type === "integer") {
    const integer = BigInt(value.value);
    return Number.isSafeInteger(Number(integer)) ? Number(integer) : integer;
  }
  return value.value;
}

export async function execute(
  database: Database,
  sql: string,
  params?: unknown[]
): Promise<QueryResult> {
  const results = await database.client().executeRaw(sql, params?.map(encode));
  const last = results.at(-1) ?? { columns: [], rows: [] };
  return {
    columns: last.columns,
    rows: last.rows.map((row) => row.map(decode)),
  };
}
