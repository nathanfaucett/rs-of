import { expect, test } from "@playwright/test";

const DISTINCTIVE = '<img src=x onerror="window.unsafeMarkup=true"> ofdb; \' persistent ✓';

async function expectReady(page: import("@playwright/test").Page): Promise<void> {
  await expect(page.getByRole("status")).toHaveText(/^Ready\./, {
    timeout: 15000,
  });
}

test.describe("ofdb SQL browser example", () => {
  test("SQL add and reload retains committed records", async ({ page }) => {
    await page.goto("/");
    await expectReady(page);
    const input = page.locator("input[placeholder='Enter record text...']");
    await input.fill(DISTINCTIVE);
    await page.getByRole("button", { name: "Add", exact: true }).click();
    await expect(page.getByRole("status")).toHaveText("Added (sync runs automatically).");

    await input.fill(`${DISTINCTIVE} second`);
    await page.getByRole("button", { name: "Add", exact: true }).click();
    await expect(page.getByRole("status")).toHaveText("Added (sync runs automatically).");

    await page.reload();
    await expectReady(page);
    const records = page.locator("ol li");
    await expect(records).toHaveCount(2);
    await expect(page.locator("ol")).toContainText(DISTINCTIVE);
    await expect(page.locator("ol")).toContainText(`${DISTINCTIVE} second`);
    await expect(page.locator("main img")).toHaveCount(0);
  });

  test("storage denied shows an error and keeps controls disabled", async ({ page }) => {
    await page.addInitScript(() => {
      indexedDB.open = () => {
        throw new DOMException("Storage denied by smoke check", "SecurityError");
      };
    });

    await page.goto("/");
    await expect(page.getByRole("status")).toHaveText(
      /^Error:.*IndexedDB open: SecurityError; allow storage access/i,
      { timeout: 15000 }
    );
    await expect(page.getByRole("button", { name: /Add Peer/ })).toBeDisabled();
  });
});
