import { expect, test } from "@playwright/test";
import { passwordManagerURL } from "./backend.mjs";

const vaultName = "Browsing test vault";
const masterPassword = "synthetic-browsing-password";

test.beforeEach(async ({ page }) => {
  await page.goto(passwordManagerURL);
  await page.getByRole("button", { name: "Create new vault", exact: true }).click();
  await page.getByLabel(/^Vault name/).fill(vaultName);
  await page.getByLabel(/^Master password/).fill(masterPassword);
  await page.getByLabel(/^Confirm password/).fill(masterPassword);
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
});

test("desktop search combines with filters and Clear resets both", async ({ page }) => {
  const search = page.getByRole("searchbox", { name: "Search items" });
  await expect(search).toHaveCount(1);
  await page.getByRole("button", { name: "New item", exact: true }).click();
  await page.getByRole("menuitem", { name: "New login" }).click();
  await page.getByRole("textbox", { name: "Name", exact: true }).fill("Synthetic login");
  await page.getByLabel("Username", { exact: true }).fill("example-user");
  await page.getByRole("button", { name: "Create item", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  const item = page.getByRole("button", { name: "Synthetic login", exact: true });
  await search.fill("no-match-for-this-query");
  await expect(item).toHaveCount(0);
  await search.fill("example-user");
  await expect(item).toBeVisible();
  const filters = page.getByRole("navigation", { name: "Filters" });
  await filters.getByRole("radio", { name: "Secure notes", exact: true }).check();
  await expect(item).toHaveCount(0);
  await page.setViewportSize({ width: 390, height: 800 });
  await expect(page.getByRole("combobox", { name: "Item type" })).toHaveValue("secureNote");
  await expect(search).toHaveValue("example-user");
  await expect(item).toHaveCount(0);
  await page.setViewportSize({ width: 1280, height: 800 });
  await filters.getByRole("button", { name: "Clear filters" }).click();
  await expect(search).toHaveValue("");
  await expect(item).toBeVisible();
});

test("Login details expose TOTP and history and retain a literal folder after reload", async ({
  page,
}) => {
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Settings" })
    .click();
  await page
    .getByLabel("Personal JSON file")
    .setInputFiles(
      new URL(
        "../apps/password-manager/src/test/fixtures/password-manager/export-expected.json",
        import.meta.url
      ).pathname
    );
  await page.getByRole("button", { name: "Validate import" }).click();
  await page.getByRole("button", { name: "Import all as new" }).click();
  await expect(
    page.getByText("Import verified. All accepted items and folders were added.")
  ).toBeVisible();
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Vault", exact: true })
    .click();

  const name = "SYNTHETIC Login café 日本語 🔐";
  const item = page.getByRole("button", { name, exact: true });
  const details = page.getByRole("region", { name: "Item details" });
  await item.press("Enter");
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  await expect(details.getByRole("heading", { name })).toBeFocused();
  await expect(details.getByRole("button", { name: "Copy TOTP code" })).toBeEnabled();
  await expect(details.getByText("SYNTHETIC-NOT-A-CREDENTIAL")).toHaveCount(0);
  await details.getByRole("button", { name: "Close details" }).click();
  await expect(item).toBeFocused();

  for (let i = 0; i < 2; i++) {
    await page.getByRole("button", { name: "New folder" }).click();
    await page.getByRole("textbox", { name: "Folder name" }).fill("Work/Shared");
    await page.getByRole("button", { name: "Save folder" }).click();
    await expect(page.getByText("Folder created.")).toBeVisible();
  }

  await item.click();
  await expect(details.getByRole("heading", { name: "Password history (2)" })).toBeVisible();
  await details.getByRole("button", { name: "Show History password 1" }).click();
  await expect(details.getByText("SYNTHETIC-PRIOR-ONE")).toBeVisible();
  const destination = details.getByRole("combobox", { name: "Move to folder" });
  await destination.selectOption({ label: "Work/Shared (2)" });
  await details.getByRole("button", { name: "Move item" }).click();
  await expect(page.getByText("Item moved.")).toBeVisible();

  await page.reload();
  await page.getByRole("button", { name: new RegExp(vaultName) }).click();
  await page.getByLabel(/^Master password/).fill(masterPassword);
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  const filters = page.getByRole("navigation", { name: "Filters" });
  await filters.getByRole("button", { name: "Work/Shared (1)", exact: true }).click();
  await expect(page.getByText("No items in this folder view.")).toBeVisible();
  await filters.getByRole("button", { name: "Work/Shared (2)", exact: true }).click();
  await item.click();
  await expect(destination.locator("option:checked")).toHaveText("Work/Shared (2)");
  await destination.selectOption({ label: "Unfiled" });
  await details.getByRole("button", { name: "Move item" }).click();
  await expect(page.getByText("Item moved.")).toBeVisible();
  await expect(item).toHaveCount(0);
  await details.getByRole("button", { name: "Close details" }).click();
  await expect(page.getByRole("searchbox", { name: "Search items" })).toBeFocused();
  await filters.getByRole("button", { name: "Clear filters" }).click();
  await item.click();
  await details.getByRole("button", { name: "Edit", exact: true }).click();
  await expect(page).toHaveURL(/\/vault\/edit\//);
  await expect(page.getByRole("textbox", { name: "Name", exact: true })).toHaveValue(name);
});

test("New item menu supports keyboard, pointer, and dismissal", async ({ page }) => {
  const trigger = page.getByRole("button", { name: "New item", exact: true });
  await expect(trigger).toBeEnabled();
  await trigger.press("ArrowDown");
  const menu = page.getByRole("menu");
  const login = menu.getByRole("menuitem", { name: "New login" });
  await expect(login).toBeFocused({ timeout: 3000 });
  await login.press("ArrowUp");
  const ssh = menu.getByRole("menuitem", { name: "New SSH key" });
  await expect(ssh).toBeFocused();
  await ssh.press("Escape");
  await expect(trigger).toBeFocused();
  await expect(menu).toHaveCount(0);
  await trigger.click();
  await expect(login).toBeFocused();
  await page.getByRole("heading", { name: "Items", exact: true }).click();
  await expect(menu).toHaveCount(0);
  await expect(trigger).toBeFocused();
  await trigger.press("ArrowDown");
  await login.press("ArrowDown");
  await menu.getByRole("menuitem", { name: "New secure note" }).press("Enter");
  await expect(page).toHaveURL(/\/vault\/new\?kind=secureNote$/);
});

test("items and details fit narrow viewports with one search and one filter presentation", async ({
  page,
}) => {
  await expect(page.getByText("No items yet.")).toBeVisible();
  await page.getByRole("link", { name: "Import items" }).click();
  await expect(page.getByRole("heading", { name: "Import Bitwarden JSON" })).toBeFocused();
  await page.goBack();
  await page.getByRole("link", { name: "New login", exact: true }).click();
  const name = `Synthetic-${"longname".repeat(14)}`;
  const website = `https://example.invalid/${"longpath".repeat(20)}`;
  const fieldName = `Synthetic-${"longfield".repeat(14)}`;
  await page.getByRole("textbox", { name: "Name", exact: true }).fill(name);
  await page.getByRole("button", { name: "Add website" }).click();
  await page.getByRole("textbox", { name: "Website 1" }).fill(website);
  await page.getByRole("button", { name: "Add custom field" }).click();
  await page.getByLabel("Field name 1").fill(fieldName);
  await page.getByLabel("Field type 1").selectOption({ label: "Hidden" });
  await page.getByLabel("Field value 1", { exact: true }).fill("SYNTHETIC-LAYOUT-SECRET");
  await page.getByRole("button", { name: "Create item", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);

  const item = page.getByRole("list", { name: "Items", exact: true }).getByRole("button", { name });
  const search = page.getByRole("searchbox", { name: "Search items" });
  const desktopFilters = page.getByRole("navigation", { name: "Filters" });
  const compactFilters = page.getByRole("group", { name: "Filters" });
  const main = page.getByRole("main");
  const checkLayout = async (width: number) => {
    await expect(search).toHaveCount(1);
    await expect(desktopFilters).toHaveCount(width >= 1024 ? 1 : 0);
    await expect(compactFilters).toHaveCount(width >= 1024 ? 0 : 1);
    await item.scrollIntoViewIfNeeded();
    await expect(item).toBeInViewport();
    await expect
      .poll(() =>
        main.evaluate((element) =>
          Math.max(
            document.documentElement.scrollWidth - document.documentElement.clientWidth,
            element.scrollWidth - element.clientWidth
          )
        )
      )
      .toBe(0);
  };

  for (const width of [320, 390, 768, 1024]) {
    await page.setViewportSize({ width, height: 800 });
    await checkLayout(width);
  }

  // Emulate the layout viewport and pixel ratio at 200%, not native browser zoom.
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setDeviceMetricsOverride", {
    width: 640,
    height: 400,
    deviceScaleFactor: 2,
    mobile: false,
  });
  await expect.poll(() => page.evaluate(() => [innerWidth, devicePixelRatio])).toEqual([640, 2]);
  await checkLayout(640);
  await cdp.send("Emulation.clearDeviceMetricsOverride");
  await cdp.detach();

  await page.setViewportSize({ width: 320, height: 800 });
  await item.click();
  const details = page.getByRole("region", { name: "Item details" });
  await expect(details.getByRole("heading", { name })).toBeFocused();
  await expect(details.getByText(website, { exact: true })).toBeVisible();
  for (const action of ["Show", "Copy"]) {
    const button = details.getByRole("button", {
      name: `${action} Custom field 1: ${fieldName}`,
      exact: true,
    });
    await expect(button).toBeVisible();
    await expect
      .poll(() =>
        button.evaluate((element) =>
          Math.max(
            element.scrollWidth - element.clientWidth,
            element.scrollHeight - element.clientHeight
          )
        )
      )
      .toBe(0);
  }
  await expect
    .poll(() => details.evaluate((element) => element.scrollWidth - element.clientWidth))
    .toBe(0);
});
