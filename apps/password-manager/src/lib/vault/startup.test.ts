// @vitest-environment playwright
import { afterEach, expect, it } from "vitest";
import { clearStorage } from "../../test/utils";
import { listVaults } from "./vault";

afterEach(async () => {
  await clearStorage();
});

it("serializes concurrent schema initialization", async () => {
  await expect(Promise.all([listVaults(), listVaults()])).resolves.toEqual([[], []]);
});
