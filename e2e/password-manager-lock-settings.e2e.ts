import { expect, type Page, test } from "@playwright/test";
import { passwordManagerURL } from "./backend.mjs";

const name = "Lock settings test vault";
const password = "synthetic-lock-settings-password";

function vaultButton(page: Page) {
  return page.getByRole("button", { name: new RegExp(`^${name}`) });
}

async function createVault(page: Page) {
  await page.goto(passwordManagerURL);
  await page.getByRole("button", { name: "Create new vault", exact: true }).click();
  await page.getByLabel(/^Vault name/).fill(name);
  await page.getByLabel(/^Master password/).fill(password);
  await page.getByLabel(/^Confirm password/).fill(password);
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
}

async function unlock(page: Page) {
  await vaultButton(page).click();
  await page.getByLabel(/^Master password/).fill(password);
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
}

test("saved inactivity timeout applies now and survives navigation, unlock, and reload", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await createVault(page);

  const navigation = page.getByRole("navigation", { name: "Main navigation" });
  const settings = navigation.getByRole("button", {
    name: "Settings",
    exact: true,
  });
  const timeout = page.getByLabel("Lock after inactivity");
  await navigation.getByRole("button", { name: "Lock vault", exact: true }).click();
  // Let IndexedDB open before advancing the clock past its open timeout.
  await expect(vaultButton(page)).toBeVisible();
  await page.clock.install({ time: new Date("2026-01-01T00:00:00Z") });
  await page.clock.pauseAt(new Date("2026-01-01T00:00:10Z"));
  await unlock(page);
  await settings.click();
  await expect(timeout).toHaveValue("5");
  await timeout.selectOption({ label: "1 minute" });
  await page.getByRole("button", { name: "Save lock settings", exact: true }).click();
  await expect(page.getByText("Lock settings saved.", { exact: true })).toBeVisible();

  await navigation.getByRole("button", { name: "Vault", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  await settings.click();
  await expect(timeout).toHaveValue("1");

  await page.clock.runFor(59_999);
  await expect(timeout).toBeVisible();
  await page.clock.runFor(1);
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
  await unlock(page);
  await settings.click();
  await expect(timeout).toHaveValue("1");

  await page.reload();
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
  await unlock(page);
  await settings.click();
  await expect(timeout).toHaveValue("1");

  await page.clock.runFor(30_000);
  await timeout.press("Tab");
  await page.clock.runFor(59_999);
  await expect(timeout).toBeVisible();
  await page.clock.runFor(1);
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
});

test("browser back retires the old deadline without locking a new session", async ({ page }) => {
  await page.clock.install({ time: new Date("2026-01-01T00:00:00Z") });
  await page.clock.pauseAt(new Date("2026-01-01T00:00:10Z"));
  await createVault(page);

  await page.clock.runFor(30_000);
  await page.goBack();
  await unlock(page);
  await page.clock.runFor(270_001);
  await expect(page.getByRole("heading", { name: "Items", exact: true })).toBeVisible();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);

  await page.clock.runFor(29_999);
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
});
