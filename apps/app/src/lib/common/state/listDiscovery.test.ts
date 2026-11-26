import { describe, expect, it } from "vitest";
import { discoverRows } from "./listDiscovery";

const rows = [
  { name: "Zulu", email: "office@example.test" },
  { name: "alpha", email: "home@example.test" },
  { name: "Bravo" },
];

describe("list discovery", () => {
  it("matches the loaded rows by name or secondary field, ignoring case and surrounding space", () => {
    expect(discoverRows(rows, " EXAMPLE ", "name-asc", (row) => [row.email])).toEqual([
      rows[1],
      rows[0],
    ]);
    expect(discoverRows(rows, " BRAV ", "default", () => [])).toEqual([rows[2]]);
    expect(discoverRows(rows, "outside this page", "default", () => [])).toEqual([]);
  });

  it("changes only the displayed order and restores owner order on request", () => {
    expect(discoverRows(rows, "", "name-desc", () => [])).toEqual([rows[0], rows[2], rows[1]]);
    expect(discoverRows(rows, "  ", "default", () => [])).toEqual([rows[0], rows[1], rows[2]]);
  });
});
