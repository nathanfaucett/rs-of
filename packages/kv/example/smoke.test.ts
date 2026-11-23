import { expect, test } from "@playwright/test";

const DISTINCTIVE = '<img src=x onerror="window.unsafeMarkup=true"> ofdb; \' persistent ✓';

async function expectReady(page: import("@playwright/test").Page): Promise<void> {
  await expect(page.getByRole("status")).toHaveText(/^Ready\./, {
    timeout: 60000,
  });
}

test.describe("ofdb KV browser example", () => {
  test("KV save, reload, and persistent value", async ({ page }) => {
    await page.goto("/");
    await expectReady(page);
    await page.getByLabel("Global Key").fill("theme");
    await page.locator("textarea").fill(DISTINCTIVE);
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByRole("status")).toHaveText("Saved (sync runs automatically).");

    await page.reload();
    await expectReady(page);
    await expect(page.locator("textarea")).toHaveValue(DISTINCTIVE);
    await expect(page.locator("main img")).toHaveCount(0);
  });

  test("KV committed value synchronizes to another peer", async ({ page }) => {
    await page.goto("/");
    await expectReady(page);
    await page.getByRole("button", { name: /Add Peer/ }).click();
    await expect(page.getByText("Peer B", { exact: true })).toBeVisible({
      timeout: 60000,
    });

    await page.getByLabel("Global Key").fill("sync-key");
    const editors = page.locator("textarea");
    await editors.nth(0).fill(DISTINCTIVE);
    await page.getByRole("button", { name: "Save", exact: true }).first().click();
    await expect(page.getByRole("status")).toHaveText("Saved (sync runs automatically).");

    await expect(editors.nth(1)).toHaveValue(DISTINCTIVE, { timeout: 60000 });
    await expect(page.getByText(/Synced at/).first()).toBeVisible({
      timeout: 60000,
    });
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
      /^Error: KV storage error \(Storage\): .*IndexedDB open: SecurityError; allow storage access/i,
      { timeout: 15000 }
    );
    await expect(page.getByRole("button", { name: /Add Peer/ })).toBeDisabled();
  });
});
