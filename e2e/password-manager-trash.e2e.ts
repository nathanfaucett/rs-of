import { expect, test } from "@playwright/test";
import { passwordManagerURL } from "./backend.mjs";

test("Trash restores one item and permanently deletes another only after confirmation", async ({
  page,
}) => {
  const vaultName = "Trash test vault";
  const password = "synthetic-trash-password";
  const restored = "Restore this login";
  const deleted = "Delete this login";
  const button = (name: string | RegExp) => page.getByRole("button", { name, exact: true });
  const row = (name: string) =>
    page.getByRole("listitem").filter({ has: page.getByText(name, { exact: true }) });

  await page.goto(passwordManagerURL);
  await button("Create new vault").click();
  await page.getByLabel(/^Vault name/).fill(vaultName);
  await page.getByLabel(/^Master password/).fill(password);
  await page.getByLabel(/^Confirm password/).fill(password);
  await button("Create vault").click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);

  for (const name of [restored, deleted]) {
    await button(/^New (item|secret)$/i).click();
    await page.getByRole("menuitem", { name: "New login", exact: true }).click();
    await page.getByLabel(/^Name/).fill(name);
    await button(/^Create (secret|item|login)$/i).click();
    const move = row(name).getByRole("button", {
      name: /^(Delete|Move to Trash)(: .+)?$/,
    });
    await move.focus();
    await move.press("Enter");
    const moveDialog = page.getByRole("dialog", { name: "Move to Trash?" });
    await expect(moveDialog).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(move).toBeFocused();
    await move.press("Enter");
    await moveDialog.getByRole("button", { name: /^(Confirm delete|Move to Trash)$/ }).click();
    await expect(page.getByText("Moved to Trash.", { exact: true })).toBeVisible();
  }

  await button("Trash").click();
  const heading = page.getByRole("heading", { name: "Trash", exact: true });
  await expect(heading).toBeVisible();
  await expect(page).toHaveTitle("Trash | Password Manager");
  await expect(button(/^New (item|secret)$/i)).toHaveCount(0);
  await expect(page.getByRole("list", { name: "Search results" })).toHaveCount(0);
  const time = row(deleted).locator("time");
  await expect(time).toHaveAttribute("datetime", /^\d{4}-\d{2}-\d{2}T.*Z$/);
  expect(await time.textContent()).not.toBe(await time.getAttribute("datetime"));

  await row(restored)
    .getByRole("button", { name: /^Restore/ })
    .click();
  await expect(page.getByText("Restored.", { exact: true })).toBeVisible();
  await expect(row(restored)).toHaveCount(0);
  const navigation = page.getByRole("navigation", { name: "Main navigation" });
  const vault = navigation.getByRole("button", { name: /^(Vault|All items)$/ });
  await vault.click();
  await expect(row(restored)).toBeVisible();
  await button("Trash").click();

  const purge = row(deleted).getByRole("button", {
    name: /^Delete permanently/,
  });
  await purge.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Delete permanently?" });
  await expect(dialog).toContainText(deleted);
  await expect(dialog).toContainText("Existing backups");
  const cancel = dialog.getByRole("button", { name: "Cancel", exact: true });
  await expect(cancel).toBeFocused();
  await cancel.click();
  await expect(purge).toBeFocused();
  await expect(dialog).toHaveCount(0);

  await purge.press("Enter");
  await expect(cancel).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(purge).toBeFocused();
  await expect(dialog).toHaveCount(0);

  await purge.press("Enter");
  await dialog.getByRole("button", { name: "Delete permanently", exact: true }).click();
  await expect(page.getByText("Permanently deleted.", { exact: true })).toBeVisible();
  await expect(heading).toBeFocused();
  await expect(page.getByText("Trash is empty.", { exact: true })).toBeVisible();

  await page.reload();
  await button(new RegExp(`^${vaultName}`)).click();
  await page.getByLabel(/^Master password/).fill(password);
  await button("Unlock").click();
  await expect(row(restored)).toBeVisible();
  await expect(row(deleted)).toHaveCount(0);
  await button("Trash").click();
  await expect(page.getByText("Trash is empty.", { exact: true })).toBeVisible();
});
