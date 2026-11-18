import { test, expect } from "@playwright/test";

const DISTINCTIVE =
  '<img src=x onerror="window.unsafeMarkup=true"> ofdb; \' persistent ✓';

test.describe("ofdb KV browser example", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector("#controls:not([disabled])", { timeout: 15000 });
  });

  test("KV save, commit, reload, retrieve", async ({ page }) => {
    // Save
    await page.fill("#key", "theme");
    await page.fill("#value", DISTINCTIVE);
    await page.click("#save");
    await expect(page.locator("#status")).toHaveText("Saved and committed.", {
      timeout: 15000,
    });

    // Reload
    await page.reload();
    await page.waitForSelector("#controls:not([disabled])", { timeout: 15000 });
    await page.click("#retrieve");
    await expect(page.locator("#status")).toHaveText(
      "Retrieved committed value.",
      { timeout: 15000 },
    );
    await expect(page.locator("#result")).toHaveText(DISTINCTIVE);
    await expect(page.locator("#result img")).toHaveCount(0);
  });

  test("storage denied shows error with recovery guidance", async ({
    page,
  }) => {
    // Block IndexedDB
    await page.addInitScript(() => {
      const original = indexedDB.open.bind(indexedDB);
      indexedDB.open = () => {
        throw new DOMException(
          "Storage denied by smoke check",
          "SecurityError",
        );
      };
    });

    await page.reload();
    await expect(page.locator("#status")).toHaveText(
      /Error:.*storage.*reload/i,
      { timeout: 15000 },
    );
    await expect(page.locator("#controls")).toBeDisabled();
  });
});
