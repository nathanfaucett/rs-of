import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";

async function download(page) {
    const pending = page.waitForEvent("download");
    await page
        .getByRole("button", { name: "Download encrypted backup", exact: true })
        .click();
    const file = await pending;
    const stream = await file.createReadStream();
    const chunks = [];
    for await (const chunk of stream) chunks.push(chunk);
    return Buffer.concat(chunks).toString("utf8");
}

async function payload(backup, password) {
    const envelope = JSON.parse(backup);
    const material = await webcrypto.subtle.importKey(
        "raw",
        new TextEncoder().encode(password),
        "PBKDF2",
        false,
        ["deriveKey"],
    );
    const key = await webcrypto.subtle.deriveKey(
        {
            name: "PBKDF2",
            hash: "SHA-512",
            iterations: 600000,
            salt: Buffer.from(envelope.kdf.salt, "base64"),
        },
        material,
        { name: "AES-GCM", length: 256 },
        false,
        ["decrypt"],
    );
    const additionalData = new TextEncoder().encode(
        JSON.stringify([
            "password-manager/encrypted-backup",
            envelope.version,
            "PBKDF2",
            "SHA-512",
            600000,
            envelope.kdf.salt,
            "AES-256-GCM",
        ]),
    );
    const plaintext = await webcrypto.subtle.decrypt(
        {
            name: "AES-GCM",
            iv: Buffer.from(envelope.nonce, "base64"),
            additionalData,
        },
        key,
        Buffer.from(envelope.ciphertext, "base64"),
    );
    return JSON.parse(new TextDecoder().decode(plaintext));
}

export async function checkBackupRestore(browser, origin) {
    const password = "synthetic-backup-master-04";
    const itemPassword = "synthetic-backup-item-04";
    const backups = [];
    const errors = [];
    const source = await browser.newContext({ acceptDownloads: true });
    source.on("weberror", () => errors.push("Source browser error"));
    try {
        const page = await source.newPage();
        page.setDefaultTimeout(15000);
        await page.goto(origin);
        await page.getByText("Offline ready", { exact: true }).waitFor();
        await source.setOffline(true);
        for (const populated of [false, true]) {
            await page
                .getByRole("button", { name: "Create new vault", exact: true })
                .click();
            await page
                .getByLabel(/^Vault name/)
                .fill(populated ? "Recovery populated" : "Recovery empty");
            await page.getByLabel(/^Master password/).fill(password);
            await page.getByLabel(/^Confirm password/).fill(password);
            await page
                .getByRole("button", { name: "Create vault", exact: true })
                .click();
            if (populated) {
                await page
                    .getByRole("button", { name: "New secret", exact: true })
                    .click();
                await page.getByLabel(/^Name/).fill("Synthetic recovery Login");
                await page
                    .getByLabel("Username", { exact: true })
                    .fill("synthetic-recovery-user-04");
                await page.getByLabel(/^Password/).fill(itemPassword);
                await page.getByLabel(/^Confirm password/).fill(itemPassword);
                await page
                    .getByRole("button", { name: "Save", exact: true })
                    .click();
                await page
                    .getByRole("button", {
                        name: "Synthetic recovery Login",
                        exact: true,
                    })
                    .waitFor();
            }
            const backup = await download(page);
            assert.ok(
                !backup.includes(password) && !backup.includes(itemPassword),
                "Encrypted download must not expose synthetic passwords",
            );
            backups.push({
                backup,
                expected: await payload(backup, password),
                populated,
            });
            await page
                .getByRole("button", { name: "Lock vault", exact: true })
                .click();
        }
    } finally {
        await source.close();
    }
    const destination = await browser.newContext({ acceptDownloads: true });
    destination.on("weberror", () => errors.push("Destination browser error"));
    try {
        const page = await destination.newPage();
        page.setDefaultTimeout(15000);
        await page.goto(origin);
        await page.getByText("Offline ready", { exact: true }).waitFor();
        await destination.setOffline(true);
        await page.reload();
        assert.equal(
            await page.getByText("Existing vaults", { exact: true }).count(),
            0,
            "New profile must not have source vault metadata",
        );
        for (const { backup, expected, populated } of backups) {
            await page
                .getByRole("button", {
                    name: "Restore encrypted backup",
                    exact: true,
                })
                .click();
            await page.getByLabel(/^Encrypted backup file/).setInputFiles({
                name: "encrypted-backup.json",
                mimeType: "application/json",
                buffer: Buffer.from(backup),
            });
            await page
                .getByLabel(/^Backup master password/)
                .fill("synthetic-wrong-password");
            await page
                .getByRole("button", { name: "Validate backup", exact: true })
                .click();
            await page
                .getByText(/Wrong password or invalid encrypted backup/)
                .waitFor();
            await page.getByLabel(/^Backup master password/).fill(password);
            await page
                .getByRole("button", { name: "Validate backup", exact: true })
                .click();
            await page
                .getByText(new RegExp(`Validated backup:.*${expected.meta.id}`))
                .waitFor();
            await page
                .getByRole("button", { name: "Restore vault", exact: true })
                .click();
            await page.getByText(/Restore verified:/).waitFor();
            await page.reload();
            await page
                .getByRole("button", {
                    name: `${expected.name} ID: ${expected.meta.id}`,
                    exact: true,
                })
                .click();
            await page.getByLabel(/^Master password/).fill(password);
            await page
                .getByRole("button", { name: "Unlock", exact: true })
                .click();
            const restoredBackup = await download(page);
            assert.deepEqual(
                await payload(restoredBackup, password),
                expected,
                "Clean-profile restore must retain the exact metadata, authentication proof, and encrypted record set",
            );
            if (populated) {
                await page
                    .getByRole("button", {
                        name: "Synthetic recovery Login",
                        exact: true,
                    })
                    .click();
                assert.equal(await page.locator("form").count(), 0);
                await page
                    .getByText("synthetic-recovery-user-04", { exact: true })
                    .last()
                    .waitFor();
                assert.equal(
                    await page.getByText(itemPassword, { exact: true }).count(),
                    0,
                );
                await page
                    .getByRole("button", { name: "Show Password", exact: true })
                    .click();
                await page.getByText(itemPassword, { exact: true }).waitFor();
                await page
                    .getByRole("button", { name: "Close details", exact: true })
                    .click();
            }
            await page
                .getByRole("button", { name: "Lock vault", exact: true })
                .click();
        }
        assert.deepEqual(errors, []);
        console.log(
            "PASS: built UI offline encrypted downloads, source-profile disposal, empty/populated clean-profile restore, wrong-password rejection, reload/unlock and exact encrypted-data/authentication identity preservation",
        );
    } finally {
        await destination.close();
    }
}
