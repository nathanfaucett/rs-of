// @vitest-environment playwright
import { afterEach, expect, it } from "vitest";
import { listVaults } from "./vault";
import { clearStorage } from "../../test/utils";

afterEach(async () => {
	await clearStorage();
});

it("serializes concurrent schema initialization", async () => {
	await expect(Promise.all([listVaults(), listVaults()])).resolves.toEqual([[], []]);
});
