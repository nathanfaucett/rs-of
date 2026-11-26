import { expect, test } from "@playwright/test";
import { passwordManagerURL } from "./backend.mjs";

test.beforeEach(async ({ page }) => {
  await page.goto(passwordManagerURL);
  await page.getByRole("button", { name: "Create new vault", exact: true }).click();
  await page.getByLabel(/^Vault name/).fill("Item details vault");
  await page.getByLabel(/^Master password/).fill("synthetic-details-password");
  await page.getByLabel(/^Confirm password/).fill("synthetic-details-password");
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  await page.getByRole("button", { name: "Settings", exact: true }).click();
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
  ).toBeFocused();
  await page.getByRole("button", { name: "Vault", exact: true }).click();
});

test.afterEach(async ({ page }) => {
  expect(await page.pageErrors()).toEqual([]);
  const messages = await page.consoleMessages();
  expect(
    messages.filter((message) => message.type() === "error").map((message) => message.text())
  ).toEqual([]);
});

const loginName = "SYNTHETIC Login café 日本語 🔐";

test("item details sit beside the list on desktop and stack at narrow widths", async ({ page }) => {
  const list = page.getByRole("region", { name: "Items", exact: true });
  const item = list.getByRole("button", { name: loginName, exact: true });
  const details = page.getByRole("region", { name: "Item details" });
  const close = details.getByRole("button", { name: "Close details" });
  const move = details.getByRole("button", { name: "Move item" });
  const edit = details.getByRole("button", { name: "Edit", exact: true });

  const checkLayout = async (split: boolean) => {
    await item.press("Enter");
    await expect(details.getByRole("heading", { name: loginName })).toBeFocused();
    const listBox = await list.boundingBox();
    const detailsBox = await details.boundingBox();
    if (!listBox || !detailsBox) throw new Error("Items or details missing");
    if (split) {
      expect(detailsBox.x).toBeGreaterThanOrEqual(listBox.x + listBox.width);
      expect(detailsBox.y).toBe(listBox.y);
      expect((await item.boundingBox())?.width).toBeGreaterThanOrEqual(120);
    } else {
      expect(detailsBox.x).toBe(listBox.x);
      expect(detailsBox.y).toBeGreaterThanOrEqual(listBox.y + listBox.height);
    }
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
      await page.evaluate(() => innerWidth)
    );
    for (const action of [close, move, edit]) {
      await action.scrollIntoViewIfNeeded();
      await expect(action).toBeInViewport({ ratio: 1 });
    }
    await close.click();
    await expect(item).toBeFocused();
  };

  for (const width of [1024, 1440, 390, 320]) {
    await page.setViewportSize({ width, height: 800 });
    await checkLayout(width >= 1024);
  }

  // Match the browsing suite's 200% layout viewport, not native browser zoom.
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setDeviceMetricsOverride", {
    width: 640,
    height: 400,
    deviceScaleFactor: 2,
    mobile: false,
  });
  await expect.poll(() => page.evaluate(() => [innerWidth, devicePixelRatio])).toEqual([640, 2]);
  await checkLayout(false);
  await cdp.detach();

  await item.click();
  const destination = details.getByRole("combobox", { name: "Move to folder" });
  await destination.selectOption({ label: "Unfiled" });
  await move.click();
  await expect(page.getByText("Item moved.")).toBeVisible();
  await expect(destination).toHaveValue("");
  await edit.click();
  await expect(page.getByRole("textbox", { name: "Name", exact: true })).toHaveValue(loginName);
});

test.describe("localized details", () => {
  test.use({ locale: "en-GB", timezoneId: "America/New_York" });

  test("displays local dates but preserves the exact timestamps", async ({ page }) => {
    await page.getByRole("button", { name: loginName, exact: true }).click();
    const details = page.getByRole("region", { name: "Item details" });
    const samples = [
      ["Created:", "2025-01-01T00:00:00.000Z", "31 Dec 2024, 19:00"],
      ["Updated:", "2025-03-01T00:00:00.000Z", "28 Feb 2025, 19:00"],
      ["Password changed at", "2025-01-02T03:04:05.000Z", "1 Jan 2025, 22:04"],
      ["Password changed at", "2025-02-03T04:05:06.000Z", "2 Feb 2025, 23:05"],
    ] as const;
    for (const [label, instant, localDate] of samples) {
      const time = details.locator(`time[datetime="${instant}"]`);
      await expect(time).toHaveText(localDate);
      await expect(details.getByText(`${label} ${localDate}`, { exact: true })).toBeVisible();
    }
  });
});

test("Launch opens only valid websites in an isolated new tab", async ({ page, context }) => {
  const item = page.getByRole("button", { name: loginName, exact: true });
  const details = page.getByRole("region", { name: "Item details" });
  const https = "https://default.example.invalid/";
  const http = "http://launch.example.invalid/sign-in";
  const malformed = "https:///missing-authority.example.invalid";
  const unsafe = "javascript:void(0)";
  const control = "https://control.example.invalid/\u0000";

  await item.click();
  await details.getByRole("button", { name: "Edit", exact: true }).click();
  for (const [index, value] of [http, malformed, unsafe, control].entries()) {
    await page.getByRole("button", { name: "Add website" }).click();
    await page.getByRole("textbox", { name: `Website ${index + 8}`, exact: true }).fill(value);
  }
  await page.getByRole("combobox", { name: "Match rule 9", exact: true }).selectOption("exact");
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  await item.click();

  const launches = details.getByRole("link", { name: /^Launch / });
  const controlWebsite = details.getByRole("group", {
    name: "Website 11",
    exact: true,
  });
  await expect(controlWebsite.getByRole("button", { name: "Copy Website 11" })).toBeEnabled();
  await expect(controlWebsite.getByRole("link")).toHaveCount(0, {
    timeout: 4000,
  });
  await expect(launches).toHaveCount(6, { timeout: 4000 });
  for (const address of [https, http]) {
    await context.route(address, (route) => route.fulfill({ body: "Synthetic sign-in page" }));
    const launch = details.getByRole("link", {
      name: `Launch ${address}`,
      exact: true,
    });
    await expect(launch).toHaveAttribute("href", address);
    const [tab] = await Promise.all([context.waitForEvent("page"), launch.click()]);
    await expect(tab).toHaveURL(address);
    expect(await tab.evaluate(() => window.opener)).toBeNull();
    await tab.close();
  }

  for (const [number, value] of [
    [6, "^https://[a-z]+\\.example\\.invalid/"],
    [7, "androidapp://invalid.example.synthetic"],
    [9, malformed],
    [10, unsafe],
  ] as const) {
    const website = details.getByRole("group", {
      name: `Website ${number}`,
      exact: true,
    });
    await expect(website.getByText(value, { exact: true })).toBeVisible();
    await expect(website.getByRole("link")).toHaveCount(0);
    await expect(website.getByRole("button", { name: `Copy Website ${number}` })).toBeEnabled();
  }
  // Both of the explicitly Exact websites still use their saved match rule.
  await expect(details.getByText("Match rule: exact", { exact: true })).toHaveCount(2);
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await details.getByRole("button", { name: "Copy Website 9", exact: true }).click();
  await expect(details.getByRole("status")).toContainText("Copied.");
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(malformed);
});
