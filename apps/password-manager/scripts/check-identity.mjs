import assert from "node:assert/strict";

const fields = {
    Title: "博士",
    "First name": "Zoë 李",
    "Middle name": "علي",
    "Last name": "O’Neill",
    "Address line 1": "東京都\n架空 001",
    "Address line 2": "Bâtiment β",
    "Address line 3": "الطابق ٢",
    City: "東京",
    "State / province": "Île-de-France",
    "Postal code": "00100-α",
    Country: "日本 / 日本国",
    Company: "Société fictive",
    Email: "synthetic (at) example.invalid",
    Phone: "+81 (0)3 0000 ext. ０１",
    "Social security number": "PRIVATE-IDENTITY-SSN-09",
    Username: "synthetic-identity-09",
    "Passport number": "PRIVATE-IDENTITY-PASSPORT-09",
    "License number": "PRIVATE-IDENTITY-LICENSE-09",
};

export async function createIdentity(page, name) {
    await page
        .getByRole("button", { name: "New identity", exact: true })
        .press("Enter");
    assert.equal(await page.getByLabel("Password", { exact: true }).count(), 0);
    await page.getByLabel(/^Name/).fill(name);
    for (const [label, value] of Object.entries(fields))
        await page.getByLabel(label, { exact: true }).fill(value);
    await page
        .getByLabel("Notes", { exact: true })
        .fill("PRIVATE-IDENTITY-NOTES 🪪\nSecond line");
    await page
        .getByRole("button", { name: "Save", exact: true })
        .press("Enter");
    await page.getByText("Saved.", { exact: true }).waitFor();
    assert.ok(!(await page.content()).includes("PRIVATE-IDENTITY"));
    await page
        .getByRole("button", { name: "Show Passport number", exact: true })
        .press("Enter");
    await page.getByText(fields["Passport number"], { exact: true }).waitFor();
    await page
        .getByRole("button", { name: "Edit", exact: true })
        .press("Enter");
    await page
        .getByLabel("First name", { exact: true })
        .fill("Discarded Identity edit");
    await page
        .getByRole("button", { name: "Cancel", exact: true })
        .press("Enter");
    assert.ok(!(await page.content()).includes("PRIVATE-IDENTITY"));
}

export async function verifyIdentity(page, name, edit = false) {
    await page.getByRole("button", { name, exact: true }).press("Enter");
    assert.ok(!(await page.content()).includes("PRIVATE-IDENTITY"));
    await page
        .getByRole("button", { name: "Edit", exact: true })
        .press("Enter");
    for (const [label, value] of Object.entries(fields))
        assert.equal(
            await page.getByLabel(label, { exact: true }).inputValue(),
            value,
        );
    assert.equal(
        await page.getByLabel("Notes", { exact: true }).inputValue(),
        "PRIVATE-IDENTITY-NOTES 🪪\nSecond line",
    );
    if (edit) {
        await page
            .getByLabel("Phone", { exact: true })
            .fill("+44 (0)20 0000 extension ００１");
        await page
            .getByRole("button", { name: "Save", exact: true })
            .press("Enter");
        await page.getByText("Saved.", { exact: true }).waitFor();
        await page
            .getByRole("button", { name: "Show Phone", exact: true })
            .press("Enter");
        await page
            .getByText("+44 (0)20 0000 extension ００１", { exact: true })
            .waitFor();
    } else
        await page
            .getByRole("button", { name: "Cancel", exact: true })
            .press("Enter");
    await page
        .getByRole("button", { name: "Close details", exact: true })
        .click();
}
