import { expect, type Page, test } from "@playwright/test";
import { passwordManagerURL } from "./backend.mjs";

async function seedPerVaultRecovery(page: Page) {
  await page.goto(passwordManagerURL);
  await expect(page.getByRole("button", { name: "Create new vault" })).toBeVisible();
  const source = await page.evaluate(async () => {
    const fixture = "/src/test/legacy-local-fixture.ts";
    const { seedLegacyLocal, legacyPassword } = await import(fixture);
    const { id, name } = await seedLegacyLocal("batch");
    return { id, name, password: legacyPassword };
  });
  await page.reload();
  return source;
}

async function unlockPerVaultChoice(page: Page, label: string, id: string, password: string) {
  await page.getByRole("button", { name: `${label} ID: ${id}`, exact: true }).click();
  await page.getByLabel("Master password", { exact: true }).fill(password);
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
}

test("a per-vault recovery backup restores and unlocks beside its retained source", async ({
  page,
}) => {
  const { id, name, password } = await seedPerVaultRecovery(page);
  const button = (name: string) => page.getByRole("button", { name, exact: true });
  const recovery = page.getByText("This recovery copy is read-only", { exact: false });
  await unlockPerVaultChoice(page, `${name} (recovery copy)`, id, password);
  await expect(recovery).toBeVisible();
  await button("Settings").click();
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    button("Download encrypted backup").click(),
  ]);
  await page.getByRole("main").getByRole("button", { name: "Lock vault", exact: true }).click();
  await button("Restore encrypted backup").click();
  await page.getByLabel(/^Encrypted backup file/).setInputFiles(await download.path());
  await page.getByLabel(/^Backup master password/).fill(password);
  await button("Validate backup").click();
  await button("Restore vault").click();
  await expect(page.getByText(/Restore verified:/)).toBeVisible();

  await page.reload();
  await unlockPerVaultChoice(page, name, id, password);
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  await expect(recovery).toHaveCount(0);
  await button("Trash").click();
  await expect(page.getByText("Legacy Login", { exact: true })).toBeVisible();
  await button("Lock vault").click();

  await unlockPerVaultChoice(page, `${name} (upgrade older vault)`, id, password);
  await expect(page.getByRole("alert")).toContainText("The shared vault does not match");
  await page.reload();
  await unlockPerVaultChoice(page, `${name} (recovery copy)`, id, password);
  await expect(recovery).toBeVisible();
});

test("an older vault upgrade can retry verification after the shared copy commits", async ({
  page,
}) => {
  const { id, name, password } = await seedPerVaultRecovery(page);
  const upgrade = `${name} (upgrade older vault)`;
  await page.evaluate(() => {
    const transaction = IDBDatabase.prototype.transaction;
    IDBDatabase.prototype.transaction = function (...args) {
      if (this.name === "password-manager-vault-registry-v1" && args[1] === "readwrite")
        throw new DOMException(
          "Fixture: legacy verification could not be saved",
          "QuotaExceededError"
        );
      return transaction.apply(this, args);
    };
  });
  await unlockPerVaultChoice(page, upgrade, id, password);
  await expect(page.getByRole("alert")).toContainText("legacy verification could not be saved");
  await page.reload();
  await expect(page.getByRole("button", { name: `${name} ID: ${id}`, exact: true })).toBeVisible();
  await unlockPerVaultChoice(page, upgrade, id, password);
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  await page.getByRole("button", { name: "Lock vault", exact: true }).click();
  await expect(page.getByRole("button", { name: `${upgrade} ID: ${id}`, exact: true })).toHaveCount(
    0
  );
});
