import { expect, test } from "@playwright/test";
import { passwordManagerURL } from "./backend.mjs";

test("Import keeps Settings open, retires the committed preview, and preserves vault data", async ({
  page,
}) => {
  const button = (name: string) => page.getByRole("button", { name, exact: true });
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  await page.goto(passwordManagerURL);
  await button("Create new vault").click();
  await page.getByLabel(/^Vault name/).fill("Import lifecycle test vault");
  await page.getByLabel(/^Master password/).fill("synthetic-import-password");
  await page.getByLabel(/^Confirm password/).fill("synthetic-import-password");
  await button("Create vault").click();
  const navigation = page.getByRole("navigation", { name: "Main navigation" });
  const items = page.getByRole("list", { name: "Items", exact: true });
  const existing = "Existing login";
  const trashed = "Trashed login";
  const imported = "Imported secure note";
  for (const name of [existing, trashed]) {
    await button("New item").click();
    await page.getByRole("menuitem", { name: "New login" }).click();
    await page.getByRole("textbox", { name: "Name", exact: true }).fill(name);
    await button("Create item").click();
    await expect(items.getByRole("button", { name, exact: true })).toBeVisible();
  }
  await items
    .getByRole("listitem")
    .filter({ has: button(trashed) })
    .getByRole("button", { name: "Delete", exact: true })
    .click();
  await page
    .getByRole("dialog", { name: "Move to Trash?" })
    .getByRole("button", { name: "Confirm delete", exact: true })
    .click();
  await expect(page.getByText("Moved to Trash.", { exact: true })).toBeVisible();
  await button("New folder").click();
  await page.getByRole("textbox", { name: "Folder name" }).fill("Existing folder");
  await button("Save folder").click();
  await expect(page.getByText("Folder created.")).toBeVisible();

  await navigation.getByRole("button", { name: "Settings" }).click();
  const timeout = page.getByLabel("Lock after inactivity");
  await timeout.selectOption("15");
  await button("Save lock settings").click();
  await expect(page.getByText("Lock settings saved.")).toBeVisible();

  const picker = page.getByLabel("Personal JSON file");
  const validate = button("Validate import");
  const preview = page.getByRole("heading", { name: "Import preview" });
  const commit = button("Import all as new");
  const validFile = {
    name: "corrected.json",
    mimeType: "application/json",
    buffer: Buffer.from(
      JSON.stringify({
        folders: [{ id: "folder-id", name: "Imported folder" }],
        items: [
          {
            id: "item-id",
            type: 2,
            name: imported,
            folderId: "folder-id",
            secureNote: { type: 0 },
          },
        ],
      })
    ),
  };
  await picker.setInputFiles({
    name: "invalid.json",
    mimeType: "application/json",
    buffer: Buffer.from("{"),
  });
  await validate.click();
  await expect(page.getByText("Import blocked", { exact: true })).toBeVisible();
  await expect(preview).toBeFocused();
  await button("Cancel import").click();
  await expect(page).toHaveURL(`${passwordManagerURL}/settings`);
  await expect(picker).toBeFocused();
  await expect(picker).toHaveValue("");
  await expect(validate).toBeDisabled();
  await expect(preview).toHaveCount(0);
  await expect(page.getByText("Import blocked", { exact: true })).toHaveCount(0);
  await expect(timeout).toHaveValue("15");

  await picker.setInputFiles(validFile);
  await validate.click();
  await expect(preview).toBeFocused();
  const result = page.getByText("Import verified. All accepted items and folders were added.");
  const destination = await page.getByText(/^Destination vault:/).innerText();
  const vaultId = destination.replace(/^Destination vault:\s*/, "");
  // Hold the real browser lock until the navigation dialog is open.
  await page.evaluate(
    (id) =>
      new Promise<void>((held, reject) => {
        void navigator.locks
          .request(
            `password-manager-import-${id}`,
            () =>
              new Promise<void>((release) => {
                document.addEventListener("release-import-lock", () => release(), { once: true });
                held();
              })
          )
          .catch(reject);
      }),
    vaultId
  );
  await page.setViewportSize({ width: 390, height: 800 });
  await commit.dblclick();
  await expect(page.getByText(/^Importing complete batch/)).toBeVisible();
  await button("Open navigation").click();
  const drawer = page.getByRole("dialog", { name: "Main navigation" });
  const settings = drawer.getByRole("button", {
    name: "Settings",
    exact: true,
  });
  await expect(settings).toBeFocused();
  await page.evaluate(() => document.dispatchEvent(new Event("release-import-lock")));
  await expect(result).toBeVisible();
  await expect(settings).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(drawer).toHaveCount(0);
  await expect(result).toBeFocused({ timeout: 4000 });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.keyboard.press("Enter");
  await expect(result).toBeFocused();
  await expect(commit).toHaveCount(0);
  await expect(preview).toHaveCount(0);
  await expect(picker).toHaveCount(0);

  await button("Import another file").click();
  await expect(picker).toBeFocused();
  await expect(picker).toHaveValue("");
  await expect(validate).toBeDisabled();
  await expect(result).toHaveCount(0);
  await picker.setInputFiles(validFile);
  await validate.click();
  await expect(page.getByText(/Source-ID matches: 1\. Logical duplicates: 1\./)).toBeVisible();
  const confirmation = page.getByRole("checkbox", {
    name: "Import duplicates as new",
  });
  await expect(confirmation).not.toBeChecked();
  await expect(commit).toBeDisabled();
  await confirmation.check();
  await expect(commit).toBeEnabled();
  await button("Cancel import").click();
  await expect(picker).toBeFocused();
  await picker.setInputFiles(validFile);
  await validate.click();
  await expect(confirmation).not.toBeChecked();
  await expect(commit).toBeDisabled();
  await button("Cancel import").click();

  await expect(timeout).toHaveValue("15");
  await navigation.getByRole("button", { name: "Vault", exact: true }).click();
  await expect(items.getByRole("listitem")).toHaveCount(2);
  await expect(items.getByRole("button", { name: existing, exact: true })).toBeVisible();
  await expect(items.getByRole("button", { name: imported, exact: true })).toHaveCount(1);
  const filters = page.getByRole("navigation", { name: "Filters" });
  await expect(filters.getByRole("button", { name: "Existing folder", exact: true })).toBeVisible();
  await filters.getByRole("button", { name: "Imported folder", exact: true }).click();
  await expect(items.getByRole("listitem")).toHaveCount(1);
  await expect(items.getByRole("button", { name: imported, exact: true })).toBeVisible();
  await navigation.getByRole("button", { name: "Trash", exact: true }).click();
  await expect(page.getByText(trashed, { exact: true })).toBeVisible();
  expect(errors).toEqual([]);
});
