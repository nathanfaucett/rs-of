import assert from "node:assert/strict";

export async function checkGenerator(page) {
    const click = (name) =>
        page.getByRole("button", { name, exact: true }).click();
    await click("Password generator");
    await click("Generate");
    await click("Show Generated value");
    const output = () =>
        page
            .getByRole("group", { name: "Generated value", exact: true })
            .locator("p")
            .first();
    assert.equal((await output().innerText()).length, 20);
    await page.getByLabel("Generate type").selectOption("passphrase");
    await page.getByLabel("Word count").fill("4");
    await click("Generate");
    await click("Show Generated value");
    assert.match(await output().innerText(), /^[a-z]+(?:-[a-z]+){3}$/);
    await page
        .getByText("Word list source and license", { exact: true })
        .click();
    await page
        .getByText("Copyright (c) 2013-2016 Pavol Rusnak", { exact: false })
        .waitFor();
    await click("New secret");
    await page.getByLabel(/^Name/).fill("Synthetic generator Login");
    await click("Password generator");
    await page.getByLabel("Generate type").selectOption("passphrase");
    await click("Generate");
    await click("Use in Login draft");
    const generated = await page
        .getByLabel("Password", { exact: true })
        .inputValue();
    assert.match(generated, /^[a-z]+(?:-[a-z]+){5}$/);
    assert.equal(
        await page
            .getByLabel("Confirm password", { exact: false })
            .inputValue(),
        generated,
    );
    await click("Save");
    await page.getByText("Saved.", { exact: true }).waitFor();
    await click("Edit");
    await click("Password generator");
    await click("Generate");
    await click("Use in Login draft");
    await click("Cancel");
    await click("Show Password");
    await page.getByText(generated, { exact: true }).waitFor();
    await click("Close details");
    console.log(
        "PASS: offline password/passphrase asset, license, explicit draft insertion, confirmation, save and cancel",
    );
    return generated;
}

export async function checkGeneratorReopened(page, generated) {
    await page
        .getByRole("button", { name: "Synthetic generator Login", exact: true })
        .click();
    assert.equal(await page.getByText(generated, { exact: true }).count(), 0);
    await page
        .getByRole("button", { name: "Show Password", exact: true })
        .click();
    await page.getByText(generated, { exact: true }).waitFor();
    await page
        .getByRole("button", { name: "Close details", exact: true })
        .click();
}
