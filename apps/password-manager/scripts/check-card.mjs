import assert from "node:assert/strict";

const number = "0000 1234 5678 0000";
const code = "007";

export async function checkCard(page, name) {
    await page.getByRole("button", { name: "Clear filters", exact: true }).press("Enter");
    await page.getByRole("button", { name: "New card", exact: true }).click();
    assert.equal(await page.getByLabel("Password", { exact: true }).count(), 0);
    await page.getByLabel(/^Name/).fill(name);
    await page.getByLabel("Cardholder name", { exact: true }).fill("山田 Synthetic");
    await page.getByLabel("Brand", { exact: true }).fill("Synthetic unknown brand");
    await page.getByLabel("Card number", { exact: true }).fill(number);
    await page.getByLabel("Expiration month", { exact: true }).fill("00");
    await page.getByLabel("Expiration year", { exact: true }).fill("1999");
    await page.getByLabel("Security code", { exact: true }).fill(code);
    await page.getByLabel("Notes", { exact: true }).fill("Synthetic Card notes\n日本語");
    assert.equal(await page.getByLabel("Card number", { exact: true }).getAttribute("type"), "password");
    assert.equal(await page.getByLabel("Security code", { exact: true }).getAttribute("type"), "password");
    await page.getByRole("button", { name: "Save", exact: true }).press("Enter");
    await page.getByText("Saved.", { exact: true }).waitFor();
    await checkCardReopened(page, name);
    await page.getByRole("button", { name: "Edit", exact: true }).press("Enter");
    await page.getByLabel("Card number", { exact: true }).fill("discarded");
    await page.getByRole("button", { name: "Add custom field", exact: true }).press("Enter");
    await page.getByLabel(/^Field name 1/).fill("Discarded field");
    await page.getByRole("button", { name: "Cancel", exact: true }).press("Enter");
    assert.equal(await page.getByText("Discarded field", { exact: true }).count(), 0);
    await checkCardReopened(page, name);
}

export async function checkCardReopened(page, name) {
    await page.getByRole("button", { name, exact: true }).press("Enter");
    await page.getByRole("button", { name: "Show Card number", exact: true }).waitFor();
    assert.equal((await page.content()).includes(number), false);
    assert.equal(await page.getByText(code, { exact: true }).count(), 0);
    await page.getByRole("button", { name: "Show Card number", exact: true }).press("Enter");
    await page.getByText(number, { exact: true }).waitFor();
    await page.getByRole("button", { name: "Show Security code", exact: true }).press("Enter");
    await page.getByText(code, { exact: true }).waitFor();
    await page.getByRole("button", { name: "Edit", exact: true }).press("Enter");
    assert.equal(await page.getByLabel("Card number", { exact: true }).inputValue(), number);
    assert.equal(await page.getByLabel("Security code", { exact: true }).inputValue(), code);
    assert.equal(await page.getByLabel("Expiration month", { exact: true }).inputValue(), "00");
    assert.equal(await page.getByLabel("Expiration year", { exact: true }).inputValue(), "1999");
    await page.getByRole("button", { name: "Cancel", exact: true }).press("Enter");
    assert.equal((await page.content()).includes(number), false);
}
