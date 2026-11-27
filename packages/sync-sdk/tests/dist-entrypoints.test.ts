import { expect, it } from "vitest";

it.each(["browser", "node"] as const)("loads the dist %s entry point", async (name) => {
  const module = await import(`../dist/${name}.js`);
  expect(module).toBeDefined();
});

it("exports the Resource lifecycle from the browser and root entry points", async () => {
  const browser = await import("../dist/browser.js");
  const root = await import("../dist/index.js");
  expect(browser.createSqlResourceSync).toBeTypeOf("function");
  expect(root.createSqlResourceSync).toBe(browser.createSqlResourceSync);
});
