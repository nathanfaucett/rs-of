import { expect, type Page, test } from "@playwright/test";
import { passwordManagerURL } from "./backend.mjs";

const vaultName = "Safe editing vault";
const masterPassword = "synthetic-master-password";
const password = "synthetic-item-password";
const totp = "JBSWY3DPEHPK3PXP";
const item = "Example login";

async function createVault(page: Page) {
  await page.goto(passwordManagerURL);
  await createFromSelector(page, vaultName);
}

async function createFromSelector(page: Page, name: string) {
  await page.getByRole("button", { name: "Create new vault", exact: true }).click();
  await page.getByLabel("Vault name", { exact: true }).fill(name);
  await page.getByLabel("Master password", { exact: true }).fill(masterPassword);
  const confirm = page.getByLabel("Confirm password", { exact: true });
  await confirm.fill(masterPassword);
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
}

async function openNew(page: Page) {
  await page.getByRole("button", { name: "New item", exact: true }).click();
  await page.getByRole("menuitem", { name: "New login" }).click();
}

async function createLogin(page: Page) {
  await openNew(page);
  await page.getByRole("textbox", { name: "Name", exact: true }).fill(item);
  await page.getByLabel("Password", { exact: true }).fill(password);
  await page.getByLabel("Confirm password", { exact: true }).fill(password);
  await page.getByLabel("Authenticator key (TOTP)", { exact: true }).fill(totp);
  await page.getByRole("button", { name: "Create item" }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
}

async function editLogin(page: Page) {
  await page.getByRole("button", { name: item, exact: true }).click();
  await page
    .getByRole("region", { name: "Item details" })
    .getByRole("button", { name: "Edit", exact: true })
    .click();
  await expect(page).toHaveURL(/\/vault\/edit\//);
}

// Keep the browser on the current page when testing a canceled unload.
async function decline(
  page: Page,
  type: "confirm" | "beforeunload",
  action: () => Promise<unknown>
) {
  const [dialog] = await Promise.all([
    page.waitForEvent("dialog", { timeout: 3000 }).then(async (dialog) => {
      await dialog.dismiss();
      return dialog;
    }),
    action(),
  ]);
  expect(dialog.type()).toBe(type);
}

test("dirty route exits warn, and declining each exit keeps the same draft", async ({ page }) => {
  await createVault(page);
  await createLogin(page);
  await editLogin(page);
  const cancel = page.getByRole("button", { name: "Cancel", exact: true });
  const name = page.getByRole("textbox", { name: "Name", exact: true });
  const cleanPrompts: string[] = [];
  const countPrompt = (dialog: import("@playwright/test").Dialog) => {
    cleanPrompts.push(dialog.type());
    void dialog.dismiss();
  };
  page.on("dialog", countPrompt);
  await cancel.click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  expect(cleanPrompts).toEqual([]);
  page.off("dialog", countPrompt);

  await editLogin(page);
  await name.fill("Edited name");
  await decline(page, "confirm", () => cancel.click());
  await decline(page, "confirm", () => page.getByRole("button", { name: "Back to vault" }).click());
  await decline(page, "confirm", () =>
    page.getByRole("button", { name: "Settings", exact: true }).click()
  );
  await decline(page, "confirm", () => page.evaluate(() => history.back()));
  await decline(page, "beforeunload", () =>
    page.evaluate(() => {
      setTimeout(() => location.reload(), 0);
    })
  );
  await expect(name).toHaveValue("Edited name");

  page.on("dialog", countPrompt);
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  await expect(page.getByRole("button", { name: "Edited name", exact: true })).toBeVisible();
  expect(cleanPrompts).toEqual([]);
  page.off("dialog", countPrompt);

  await openNew(page);
  await name.fill("Abandoned draft");
  const accepted: string[] = [];
  page.on("dialog", async (dialog) => {
    accepted.push(dialog.type());
    await dialog.accept();
  });
  await cancel.click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  expect(accepted).toEqual(["confirm"]);
  await expect(page.getByRole("button", { name: "Abandoned draft", exact: true })).toHaveCount(0);
});

async function selectVault(page: Page) {
  await page.getByRole("button", { name: new RegExp(vaultName) }).click();
  return page.getByLabel("Master password", { exact: true });
}

test("create and unlock use native keyboard submission and password help", async ({ page }) => {
  await page.goto(passwordManagerURL);
  await page.getByRole("button", { name: "Create new vault", exact: true }).click();
  await expect(page).toHaveTitle("Create vault | Password Manager");
  const master = page.getByLabel("Master password", { exact: true });
  const confirm = page.getByLabel("Confirm password", { exact: true });
  await expect(master).toHaveAccessibleDescription("Use at least 8 characters.");
  await expect(master).toHaveAttribute("autocomplete", "new-password");
  await expect(confirm).toHaveAttribute("autocomplete", "new-password");
  await page.getByLabel("Vault name", { exact: true }).fill(vaultName);
  await master.fill("short");
  await confirm.fill("short");
  await confirm.press("Enter");
  await expect(page.getByRole("alert")).toContainText("at least 8 characters");
  await master.fill(masterPassword);
  await confirm.fill(masterPassword);
  await confirm.press("Enter");
  await expect(page).toHaveTitle("Items | Password Manager");

  await page.getByRole("button", { name: "Lock vault" }).click();
  const unlock = await selectVault(page);
  await expect(page).toHaveTitle("Unlock vault | Password Manager");
  await expect(unlock).toHaveAttribute("autocomplete", "current-password");
  await unlock.fill("wrong-password");
  await unlock.press("Enter");
  await expect(page.getByRole("alert")).toContainText("Wrong password");
  await unlock.fill(masterPassword);
  await unlock.press("Enter");
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
});

test("details and editor mask secrets independently", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await createVault(page);
  await createLogin(page);
  await page.getByRole("button", { name: item, exact: true }).click();
  await expect(page.getByText(password, { exact: true })).toHaveCount(0);
  await expect(page.getByText(totp, { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Copy Password", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "Copied." })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(password);
  await page.getByRole("button", { name: "Show Password", exact: true }).click();
  await expect(page.getByText(password, { exact: true })).toBeVisible();
  await expect(page.getByText(totp, { exact: true })).toHaveCount(0);
  await page
    .getByRole("region", { name: "Item details" })
    .getByRole("button", { name: "Edit", exact: true })
    .click();
  await expect(page).toHaveTitle("Edit item | Password Manager");
  const passwordInput = page.getByLabel("Password", { exact: true });
  const totpInput = page.getByLabel("Authenticator key (TOTP)", {
    exact: true,
  });
  const confirmation = page.getByLabel("Confirm password", { exact: true });
  for (const input of [passwordInput, totpInput, confirmation]) {
    await expect(input).toHaveAttribute("type", "password");
  }
  await page.getByRole("button", { name: "Show Password", exact: true }).click();
  await expect(page.getByRole("button", { name: "Hide Password", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true"
  );
  await expect(passwordInput).toHaveAttribute("type", "text");
  await expect(totpInput).toHaveAttribute("type", "password");
  await expect(confirmation).toHaveAttribute("type", "password");
});

// Delay the first read-back of the encrypted save, after the real local write.
async function pauseSaveAcknowledgment(page: Page) {
  let acknowledge = () => {};
  let release = () => {};
  const reached = new Promise<void>((resolve) => {
    acknowledge = resolve;
  });
  const pause = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.exposeFunction("pauseAcknowledgment", () => {
    acknowledge();
    return pause;
  });
  await page.evaluate(() => {
    const subtle = crypto.subtle;
    const encrypt = subtle.encrypt.bind(subtle);
    const decrypt = subtle.decrypt.bind(subtle);
    subtle.encrypt = async (...args) => {
      subtle.encrypt = encrypt;
      const encrypted = await encrypt(...args);
      const ciphertext = new Uint8Array(encrypted);
      subtle.decrypt = async (algorithm, key, data) => {
        const bytes = ArrayBuffer.isView(data)
          ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
          : new Uint8Array(data);
        if (
          bytes.length === ciphertext.length &&
          bytes.every((byte, i) => byte === ciphertext[i])
        ) {
          subtle.decrypt = decrypt;
          await (
            window as unknown as { pauseAcknowledgment(): Promise<void> }
          ).pauseAcknowledgment();
        }
        return decrypt(algorithm, key, data);
      };
      return encrypted;
    };
  });
  return { reached, release };
}

for (const lock of ["manual", "inactivity"] as const) {
  test(`${lock} lock retires an editor with a save awaiting acknowledgment`, async ({ page }) => {
    await page.clock.install();
    await createVault(page);
    await openNew(page);
    const name = page.getByRole("textbox", { name: "Name", exact: true });
    await name.fill("Save in progress");
    const paused = await pauseSaveAcknowledgment(page);
    const prompts: string[] = [];
    page.on("dialog", async (dialog) => {
      prompts.push(dialog.type());
      await dialog.dismiss();
    });
    try {
      await page.getByRole("button", { name: "Create item" }).click();
      await paused.reached;
      await expect(page.getByRole("button", { name: "Saving...", exact: true })).toBeDisabled();
      await page.getByRole("button", { name: "Settings", exact: true }).click();
      await expect(name).toHaveValue("Save in progress");
      if (lock === "manual") {
        await page.getByRole("button", { name: "Lock vault" }).click();
      } else {
        await page.clock.fastForward(6 * 60_000);
      }
      await expect(page).toHaveURL(`${passwordManagerURL}/`);
      await expect(page.getByRole("button", { name: "Create new vault" })).toBeVisible();
      await expect(name).toHaveCount(0);
      expect(prompts).toEqual([]);

      if (lock === "manual") {
        await createFromSelector(page, "New vault session");
        await openNew(page);
        await name.fill("New session draft");
        paused.release();
        await page.getByRole("button", { name: "Create item" }).click();
        await expect(
          page.getByRole("button", { name: "New session draft", exact: true })
        ).toBeVisible();
        await expect(
          page.getByRole("button", { name: "Save in progress", exact: true })
        ).toHaveCount(0);
        await page.getByRole("button", { name: "Lock vault" }).click();
      } else {
        paused.release();
      }
      const unlock = await selectVault(page);
      await unlock.fill(masterPassword);
      await unlock.press("Enter");
      await expect(
        page.getByRole("button", { name: "Save in progress", exact: true })
      ).toBeVisible();
      expect(prompts).toEqual([]);
    } finally {
      paused.release();
    }
  });
}

test("card, identity, SSH, and hidden custom secrets open masked", async ({ page }) => {
  await createVault(page);
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page).toHaveTitle("Settings | Password Manager");
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
  await page.getByRole("button", { name: "Vault", exact: true }).click();

  const samples = [
    ["SYNTHETIC Card (not usable)", "Card number", "Card number", "0000000000000000"],
    [
      "SYNTHETIC Identity",
      "Social security number",
      "Social security number",
      "SYNTHETIC-NOT-AN-SSN",
    ],
    [
      "SYNTHETIC Login café 日本語 🔐",
      "Custom field 2: hidden",
      "Field value 2",
      "NOT-A-REAL-SECRET",
    ],
    [
      "SYNTHETIC SSH (text storage only)",
      "Private key",
      "Private key",
      "SYNTHETIC PRIVATE KEY TEXT\nNot an SSH key.\n",
    ],
  ] as const;
  for (const [record, detailLabel, editorLabel, value] of samples) {
    await page.getByRole("button", { name: record, exact: true }).click();
    const details = page.getByRole("region", { name: "Item details" });
    await expect(
      details.getByRole("button", { name: `Show ${detailLabel}`, exact: true })
    ).toBeVisible();
    await expect(details).not.toContainText(value);
    await details.getByRole("button", { name: "Edit", exact: true }).click();
    const input = page.getByLabel(editorLabel, { exact: true });
    const show = page.getByRole("button", {
      name: `Show ${editorLabel}`,
      exact: true,
    });
    if (editorLabel === "Private key") {
      await expect(page).toHaveTitle("Edit item | Password Manager");
      await expect(input).toHaveCount(0);
      await show.focus();
      await expect(show).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(input).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(
        page.getByRole("button", { name: `Copy ${editorLabel}`, exact: true })
      ).toBeFocused();
    } else {
      await expect(input).toHaveAttribute("type", "password");
      await show.click();
    }
    await expect(input).toHaveValue(value);
    await expect(input).toHaveAccessibleName(editorLabel);
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
  }
});

test("changing a hidden custom field to Boolean does not reveal its old value", async ({
  page,
}) => {
  const secret = "SYNTHETIC-HIDDEN-CUSTOM-VALUE";
  await createVault(page);
  await openNew(page);
  await page.getByRole("textbox", { name: "Name", exact: true }).fill(item);
  await page.getByRole("button", { name: "Add custom field" }).click();
  await page.getByLabel("Field name 1", { exact: true }).fill("Example flag");
  const type = page.getByLabel("Field type 1", { exact: true });
  const value = page.getByLabel("Field value 1", { exact: true });
  await type.selectOption({ label: "Hidden" });
  await value.fill(secret);

  await type.selectOption({ label: "Boolean" });
  await expect(value.locator("option")).toHaveText(["Select True or False", "True", "False"]);
  await expect(value).toHaveValue("");
  await value.selectOption({ label: "False" });
  await page.getByRole("button", { name: "Create item" }).click();
  await page.getByRole("button", { name: item, exact: true }).click();
  const details = page.getByRole("region", { name: "Item details" });
  await expect(details.getByText("false", { exact: true })).toBeVisible();
  await expect(details).not.toContainText(secret);
});
