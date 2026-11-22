import { expect, it } from "vitest";

it.each(["browser", "node"] as const)("loads the dist %s entry point", async (name) => {
  const module = await import(`../dist/${name}.js`);
  expect(module).toBeDefined();
});
