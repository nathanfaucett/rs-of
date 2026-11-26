import { expect, type Locator, type Page, test } from "@playwright/test";
import { passwordManagerURL } from "./backend.mjs";

const masterPassword = "synthetic-master-password";

async function createVault(page: Page) {
  await page.goto(passwordManagerURL);
  await page.getByRole("button", { name: "Create new vault", exact: true }).click();
  await page.getByLabel("Vault name", { exact: true }).fill("Item editor vault");
  await page.getByLabel("Master password", { exact: true }).fill(masterPassword);
  await page.getByLabel("Confirm password", { exact: true }).fill(masterPassword);
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);
}

async function startItem(page: Page, kind: string, name: string) {
  await page.getByRole("button", { name: "New item", exact: true }).click();
  await page.getByRole("menuitem", { name: `New ${kind}`, exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill(name);
}

async function openDetails(page: Page, name: string) {
  await page.getByRole("button", { name, exact: true }).click();
  return page.getByRole("region", { name: "Item details" });
}

test("Secure Note text is immediately editable but masked in saved details", async ({ page }) => {
  const name = "Example secure note";
  const original = "Synthetic private note\nSecond line";
  const updated = "Updated synthetic private note\nSecond line";
  const editor = page.getByRole("textbox", { name: "Notes", exact: true });

  await createVault(page);
  await startItem(page, "secure note", name);
  await expect(editor).toBeVisible();
  await editor.fill(original);
  await page.getByRole("button", { name: "Create item", exact: true }).click();

  const details = await openDetails(page, name);
  await expect(details.getByText(original)).toHaveCount(0);
  await expect(details.getByRole("button", { name: "Show Notes", exact: true })).toBeVisible();
  await details.getByRole("button", { name: "Edit", exact: true }).click();
  await expect(editor).toBeVisible();
  await expect(editor).toHaveValue(original);
  await editor.fill(updated);
  await page.getByRole("button", { name: "Save changes", exact: true }).click();

  await openDetails(page, name);
  await expect(details.getByText(updated)).toHaveCount(0);
  await details.getByRole("button", { name: "Show Notes", exact: true }).click();
  await expect(details.getByText(updated, { exact: true })).toBeVisible();
});

const editorCases = [
  {
    kind: "card",
    ordinary: ["Cardholder name", "Brand", "Expiration month", "Expiration year", "Notes"],
    protected: ["Card number", "Security code"],
  },
  {
    kind: "identity",
    ordinary: [
      "Title",
      "First name",
      "Middle name",
      "Last name",
      "Address line 1",
      "Address line 2",
      "Address line 3",
      "City",
      "State / province",
      "Postal code",
      "Country",
      "Company",
      "Email",
      "Phone",
      "Username",
      "Notes",
    ],
    protected: ["Social security number", "Passport number", "License number"],
  },
] as const;

for (const fields of editorCases) {
  test(`${fields.kind} fields persist edits without browser ownership warnings`, async ({
    page,
    context,
  }) => {
    const diagnostics: string[] = [];
    page.on("pageerror", (error) => diagnostics.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error" || message.text().includes("ownership_invalid_")) {
        diagnostics.push(message.text());
      }
    });
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    const name = `Example ${fields.kind}`;
    const ordinary = fields.ordinary.map((label) => ({
      label,
      input: page.getByRole("textbox", { name: label, exact: true }),
    }));
    const protectedInputs = fields.protected.map((label) => ({
      label,
      input: page.getByLabel(label, { exact: true }),
    }));
    const inputs = [...ordinary, ...protectedInputs];
    const [firstSecret] = fields.protected;
    const secret = page.getByLabel(firstSecret, { exact: true });
    const edit = (details: Locator) =>
      details.getByRole("button", { name: "Edit", exact: true }).click();
    const original = (label: string) => `Synthetic original ${label}`;
    const updated = (label: string) =>
      `Synthetic updated ${label}${label.startsWith("Address") || label === "Notes" ? "\nSecond line" : ""}`;

    await createVault(page);
    await startItem(page, fields.kind, name);
    for (const { label, input } of inputs) {
      await expect(input).toBeVisible();
      await input.fill(original(label));
    }
    for (const { input } of protectedInputs)
      await expect(input).toHaveAttribute("type", "password");
    await page.getByRole("button", { name: "Create item", exact: true }).click();

    await edit(await openDetails(page, name));
    for (const { label, input } of inputs) {
      await expect(input).toHaveValue(original(label));
      await input.fill(updated(label));
    }
    for (const { input } of protectedInputs)
      await expect(input).toHaveAttribute("type", "password");
    await page.getByRole("button", { name: `Show ${firstSecret}`, exact: true }).click();
    await expect(secret).toHaveAttribute("type", "text");
    for (const { input } of protectedInputs.slice(1))
      await expect(input).toHaveAttribute("type", "password");
    await page.getByRole("button", { name: `Copy ${firstSecret}`, exact: true }).click();
    await expect(page.getByRole("status").filter({ hasText: "Copied." })).toBeVisible();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(updated(firstSecret));
    await page.getByRole("button", { name: `Hide ${firstSecret}`, exact: true }).click();
    await expect(secret).toHaveAttribute("type", "password");
    await page.getByRole("button", { name: "Save changes", exact: true }).click();

    const details = await openDetails(page, name);
    const masked = fields.kind === "identity" ? [...fields.protected, "Notes"] : fields.protected;
    for (const label of masked) {
      await expect(
        details.getByRole("button", { name: `Show ${label}`, exact: true })
      ).toBeVisible();
      await expect(details.getByText(updated(label), { exact: true })).toHaveCount(0);
    }
    await edit(details);
    for (const { label, input } of inputs) await expect(input).toHaveValue(updated(label));
    expect(diagnostics).toEqual([]);
  });
}
