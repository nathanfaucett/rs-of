import { expect, type Locator, test } from "@playwright/test";
import { passwordManagerURL } from "./backend.mjs";

test.beforeEach(async ({ page }) => {
  await page.goto(passwordManagerURL);
  await page.getByRole("button", { name: "Create new vault", exact: true }).click();
  await page.getByLabel(/^Vault name/).fill("Navigation test vault");
  await page.getByLabel(/^Master password/).fill("synthetic-navigation-password");
  await page.getByLabel(/^Confirm password/).fill("synthetic-navigation-password");
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
});

test("mobile navigation contains focus and closes by keyboard, pointer, route, or resize", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });

  const nav = page.getByRole("navigation", { name: "Main navigation" });
  const expand = nav.getByRole("button", { name: "Expand navigation" });
  await nav.getByRole("button", { name: "Collapse navigation" }).click();
  await expect(expand).toBeVisible();
  await page.setViewportSize({ width: 390, height: 800 });
  const opener = page.getByRole("button", { name: "Open navigation" });
  const dialog = page.getByRole("dialog", { name: "Main navigation" });
  const close = dialog.getByRole("button", { name: "Close navigation" });
  const vault = dialog.getByRole("button", { name: "Vault", exact: true });
  const settings = dialog.getByRole("button", {
    name: "Settings",
    exact: true,
  });
  const lock = dialog.getByRole("button", { name: "Lock vault" });

  await opener.press("Enter");
  await expect(vault).toBeFocused();
  const cdp = await page.context().newCDPSession(page);
  const { nodes } = await cdp.send("Accessibility.getFullAXTree");
  expect(nodes.some((node) => !node.ignored && node.role?.value === "main")).toBe(false);
  await cdp.detach();
  await page.getByRole("searchbox", { name: "Search items", includeHidden: true }).focus();
  await expect(vault).toBeFocused();

  await page.keyboard.press("ArrowDown");
  await expect(dialog.getByRole("button", { name: "Generator" })).toBeFocused();
  await page.keyboard.press("End");
  await expect(dialog.getByRole("button", { name: "Trash" })).toBeFocused();
  await page.keyboard.press("ArrowUp");
  await expect(settings).toBeFocused();
  await page.keyboard.press("Home");
  await expect(vault).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(lock).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(vault).toBeFocused();
  await dialog.getByRole("link", { name: "Sign in to sync" }).press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(opener).toBeFocused();

  await opener.click();
  await close.press("Enter");
  await expect(dialog).toHaveCount(0);
  await expect(opener).toBeFocused();

  await opener.click();
  await page.mouse.click(380, 400);
  await expect(dialog).toHaveCount(0);
  await expect(opener).toBeFocused();

  await opener.click();
  await settings.press("Enter");
  await expect(page).toHaveURL(`${passwordManagerURL}/settings`);
  await expect(dialog).toHaveCount(0);
  await opener.click();
  await expect(settings).toBeFocused();
  await page.setViewportSize({ width: 1024, height: 800 });
  await expect(dialog).toHaveCount(0);
  await expect(expand).toBeVisible();
  await expect(nav.getByRole("button", { name: "Settings", exact: true })).toBeFocused();
  await page.setViewportSize({ width: 390, height: 800 });
  await expect(opener).toBeVisible();
  await expect(dialog).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("the mobile opener does not cover vault or editor controls", async ({ page }) => {
  await page.getByRole("link", { name: "New login", exact: true }).click();
  await page.getByRole("textbox", { name: "Name", exact: true }).fill("Synthetic record");
  await page.getByRole("button", { name: "Create item", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);

  const opener = page.getByRole("button", { name: "Open navigation" });
  const record = page.getByRole("button", {
    name: "Synthetic record",
    exact: true,
  });
  const newItem = page.getByRole("button", { name: "New item", exact: true });
  const sort = page.getByRole("combobox", { name: "Sort items" });
  const count = page.getByText("1 item found.", { exact: true });
  async function expectNoOverlap(
    target: Locator,
    width: number,
    block: ScrollLogicalPosition = "end"
  ) {
    await target.evaluate((element, block) => element.scrollIntoView({ block }), block);
    await expect(target).toBeInViewport();
    const button = await opener.boundingBox();
    const control = await target.boundingBox();
    if (!button || !control) throw new Error(`Missing layout box at ${width}px`);
    expect(
      button.x < control.x + control.width &&
        button.x + button.width > control.x &&
        button.y < control.y + control.height &&
        button.y + button.height > control.y,
      `navigation opener intersects ${target} at ${width}px`
    ).toBe(false);
  }

  for (const width of [320, 390, 768]) {
    await page.setViewportSize({ width, height: 320 });
    await expectNoOverlap(newItem, width);
    await expect(opener).toBeInViewport();
    for (const target of [sort, count, record]) await expectNoOverlap(target, width);
    await expectNoOverlap(record, width, "start");
  }

  await newItem.click();
  await page.getByRole("menuitem", { name: "New login" }).click();
  await expect(page).toHaveURL(/\/vault\/new\?kind=login$/);
  for (const width of [320, 390, 768]) {
    await page.setViewportSize({ width, height: 320 });
    await expectNoOverlap(page.getByRole("textbox", { name: "Name", exact: true }), width);
    await expectNoOverlap(page.getByRole("button", { name: "Create item", exact: true }), width);
  }
});
