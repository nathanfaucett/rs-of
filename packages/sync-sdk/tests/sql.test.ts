import type { Database, Result } from "@ofsdk/sql";
import { expect, it, vi } from "vitest";
import { execute } from "../src/sql";

function stub(results: Result[]) {
  const executeRaw = vi.fn(async () => results);
  const database = {
    client: () => ({ executeRaw }),
  } as unknown as Database;
  return { database, executeRaw };
}

it("uses executeRaw with tagged parameters and decodes the last result", async () => {
  const { database, executeRaw } = stub([
    {
      columns: ["value", "safe", "large"],
      rows: [
        [
          { type: "null" },
          { type: "integer", value: "1" },
          { type: "integer", value: "9007199254740992" },
        ],
      ],
    },
  ]);

  const result = await execute(database, "SELECT ?", [null, "text", false, 7, 8n]);

  expect(executeRaw).toHaveBeenCalledWith("SELECT ?", [
    { type: "null" },
    { type: "text", value: "text" },
    { type: "bool", value: false },
    { type: "integer", value: "7" },
    { type: "integer", value: "8" },
  ]);
  expect(result).toEqual({
    columns: ["value", "safe", "large"],
    rows: [[null, 1, 9007199254740992n]],
  });
});

it("returns an empty result when SQL returns no statements", async () => {
  const { database, executeRaw } = stub([]);

  await expect(execute(database, "SELECT 1")).resolves.toEqual({
    columns: [],
    rows: [],
  });
  expect(executeRaw).toHaveBeenCalledWith("SELECT 1", undefined);
});

it("rejects unsupported parameters before executing SQL", async () => {
  const { database, executeRaw } = stub([]);

  await expect(execute(database, "SELECT ?", [1.5])).rejects.toThrow(
    "SQL parameters must be text, booleans, integers or null"
  );
  expect(executeRaw).not.toHaveBeenCalled();
});
