import assert from "node:assert/strict";

const name = "Synthetic offline SSH Key";
const privateKey =
    "  -----BEGIN OPENSSH PRIVATE KEY-----\nPRIVATE-OFFLINE-SSH 秘密\nnot-a-functional-key\n-----END OPENSSH PRIVATE KEY-----\n  ";
const publicKey = "ssh-ed25519 SYNTHETIC-not-a-key comment 秘密\n";
const fingerprint = "SHA256:synthetic-not-derived";
const button = (page, name) => page.getByRole("button", { name, exact: true });

export async function createSshKey(page) {
    await button(page, "New SSH key").press("Enter");
    await page.getByLabel(/^Name/).fill(name);
    assert.equal(await page.getByLabel("Password", { exact: true }).count(), 0);
    assert.equal(await page.locator("#private-key").count(), 0);
    await button(page, "Show Private key").press("Enter");
    await page.getByLabel(/^Private key/).fill(privateKey);
    await page.getByLabel("Public key", { exact: true }).fill(publicKey);
    await page.getByLabel("Key fingerprint", { exact: true }).fill(fingerprint);
    await button(page, "Save").press("Enter");
    await page.getByText("Saved.", { exact: true }).waitFor();
    await verifySshKey(page);
}

export async function verifySshKey(page) {
    await button(page, "Clear filters").press("Enter");
    await button(page, name).press("Enter");
    assert.equal((await page.content()).includes("PRIVATE-OFFLINE-SSH"), false);
    await button(page, "Show Private key").press("Enter");
    assert.equal(
        await page.getByText(/PRIVATE-OFFLINE-SSH/).textContent(),
        privateKey,
    );
    await button(page, "Edit").press("Enter");
    assert.equal(await page.locator("#private-key").count(), 0);
    await button(page, "Show Private key").press("Enter");
    assert.equal(
        await page.getByLabel(/^Private key/).inputValue(),
        privateKey,
    );
    assert.equal(
        await page.getByLabel("Public key", { exact: true }).inputValue(),
        publicKey,
    );
    assert.equal(
        await page.getByLabel("Key fingerprint", { exact: true }).inputValue(),
        fingerprint,
    );
    await page.getByLabel(/^Private key/).fill("PRIVATE-ABANDONED-SSH");
    await button(page, "Cancel").press("Enter");
    assert.equal((await page.content()).includes("PRIVATE-"), false);
    await button(page, "Edit").press("Enter");
    await page
        .getByLabel("Notes", { exact: true })
        .fill("SSH metadata 秘密\nPreserve the key text");
    await button(page, "Save").press("Enter");
    await page.getByText("Saved.", { exact: true }).waitFor();
    await button(page, "Show Private key").press("Enter");
    assert.equal(
        await page.getByText(/PRIVATE-OFFLINE-SSH/).textContent(),
        privateKey,
    );
    await button(page, "Close details").press("Enter");
    console.log(
        "PASS: offline SSH keyboard create/view/edit/cancel/save, exact multiline key fields, and protected reveal reset",
    );
}
