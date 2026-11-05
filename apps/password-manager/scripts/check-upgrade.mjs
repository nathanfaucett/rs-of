import assert from "node:assert/strict";
import { createCipheriv, pbkdf2Sync, randomBytes } from "node:crypto";
import { download, logicalPayload, payload } from "./check-backup.mjs";

const button = (page, name) => page.getByRole("button", { name, exact: true });
const password = "synthetic-upgrade-20";
const date = "2025-01-02T03:04:05.000Z";
const login = {
    id: "30000000-0000-4000-8000-000000000001",
    name: "Legacy qualification 日本語",
    username: "0001",
    secret: "PRIVATE-LEGACY-20",
    totp: "JBSWY3DPEHPK3PXP",
    notes: "Legacy café\nSecond line",
    favorite: true,
    uris: [
        { uri: "example.invalid" },
        { uri: "androidapp://synthetic", match: "never" },
    ],
    fields: [
        { name: "PIN", type: "hidden", value: "0000" },
        { name: "Enabled", type: "boolean", value: "false" },
    ],
    history: [{ secret: "PRIVATE-LEGACY-HISTORY", changedAt: date }],
    createdAt: date,
    updatedAt: date,
};

// Historical version-1 wire fixture, not the current application's serializer.
function fixture() {
    const salt = randomBytes(16).toString("base64");
    const key = pbkdf2Sync(
        password,
        Buffer.from(salt, "base64"),
        600000,
        32,
        "sha512",
    );
    const meta = {
        id: "30000000-0000-4000-8000-000000000020",
        salt,
        iv: randomBytes(12).toString("base64"),
        createdAt: date,
    };
    function encrypt(value, aad) {
        const nonce = randomBytes(12);
        const cipher = createCipheriv("aes-256-gcm", key, nonce);
        if (aad) cipher.setAAD(Buffer.from(JSON.stringify(aad)));
        return {
            nonce: nonce.toString("base64"),
            ciphertext: Buffer.concat([
                cipher.update(JSON.stringify(value)),
                cipher.final(),
                cipher.getAuthTag(),
            ]).toString("base64"),
        };
    }
    const authentication = JSON.stringify({
        version: 1,
        ...encrypt([
            "password-manager/vault-authentication",
            1,
            meta.id,
            salt,
            meta.iv,
            date,
        ]),
    });
    const records = [
        login,
        {
            ...login,
            id: "30000000-0000-4000-8000-000000000002",
            name: "Legacy Trash",
            deletedAt: date,
        },
    ];
    const original = {
        meta,
        authentication,
        name: "Legacy upgrade fixture",
        lockSettings: null,
        secrets: records.map((item) => ({
            id: item.id,
            ...encrypt(item),
            createdAt: date,
            updatedAt: date,
        })),
    };
    const envelope = {
        format: "password-manager/encrypted-backup",
        version: 1,
        kdf: { name: "PBKDF2", hash: "SHA-512", iterations: 600000, salt },
        cipher: "AES-256-GCM",
        ...encrypt(original, [
            "password-manager/encrypted-backup",
            1,
            "PBKDF2",
            "SHA-512",
            600000,
            salt,
            "AES-256-GCM",
        ]),
    };
    return { backup: JSON.stringify(envelope), original };
}

async function fileDownload(page, name) {
    const pending = page.waitForEvent("download");
    await button(page, name).click();
    const chunks = [];
    for await (const chunk of await (await pending).createReadStream())
        chunks.push(chunk);
    return Buffer.concat(chunks).toString("utf8");
}

export async function checkLegacyUpgrade(browser, origin) {
    const context = await browser.newContext({ acceptDownloads: true });
    const errors = [];
    context.on("weberror", () =>
        errors.push("Unhandled upgrade browser error"),
    );
    try {
        const page = await context.newPage();
        await page.goto(origin);
        await page.getByText("Offline ready", { exact: true }).waitFor();
        await context.setOffline(true);
        await page.reload();
        const { backup, original } = fixture();
        assert.deepEqual(await payload(backup, password), original);
        await button(page, "Restore encrypted backup").press("Enter");
        await page.getByLabel(/^Encrypted backup file/).setInputFiles({
            name: "legacy-v1.json",
            mimeType: "application/json",
            buffer: Buffer.from(backup),
        });
        await page.getByLabel(/^Backup master password/).fill(password);
        await button(page, "Validate backup").press("Enter");
        await page.getByText(/Validated backup:/).waitFor();
        await button(page, "Restore vault").press("Enter");
        await page.getByText(/Restore verified:/).waitFor();
        await page.reload();
        await button(page, `${original.name} ID: ${original.meta.id}`).press(
            "Enter",
        );
        await page
            .getByLabel(/^Master password/)
            .fill("wrong-upgrade-password");
        await button(page, "Unlock").press("Enter");
        await page
            .getByRole("alert")
            .filter({ hasText: /Wrong password/ })
            .waitFor();
        await page.getByLabel(/^Master password/).fill(password);
        await button(page, "Unlock").press("Enter");
        await button(page, `★ ${login.name}`).waitFor();
        const current = await download(page);
        const upgraded = await payload(current, password);
        assert.equal(JSON.parse(current).version, 3);
        assert.equal(upgraded.itemFormat, 2);
        assert.equal(upgraded.secrets.length, 2);
        assert.deepEqual(upgraded.meta, original.meta);
        assert.equal(upgraded.authentication, original.authentication);
        const logical = await logicalPayload(current, password);
        assert.deepEqual(
            logical.items,
            [
                login,
                {
                    ...login,
                    id: "30000000-0000-4000-8000-000000000002",
                    name: "Legacy Trash",
                    deletedAt: date,
                },
            ].map((item) => ({
                ...item,
                kind: "login",
                uris: [
                    { uri: "example.invalid", match: "domain" },
                    item.uris[1],
                ],
            })),
        );
        assert.deepEqual(logical.folders, []);
        const retained = await fileDownload(
            page,
            "Download pre-transition backup",
        );
        assert.deepEqual(await payload(retained, password), original);
        await button(page, `★ ${login.name}`).press("Enter");
        assert.equal((await page.content()).includes(login.secret), false);
        await button(page, "Show Password").press("Enter");
        await page.getByText(login.secret, { exact: true }).waitFor();
        await button(page, "Edit").press("Enter");
        assert.equal(
            await page.getByLabel("Username", { exact: true }).inputValue(),
            login.username,
        );
        assert.equal(
            await page.getByLabel("Notes", { exact: true }).inputValue(),
            login.notes,
        );
        await button(page, "Cancel").press("Enter");
        await button(page, "Trash").press("Enter");
        await page.getByText("Legacy Trash", { exact: true }).waitFor();
        await button(page, "Lock vault").press("Enter");
        await page.reload();
        await button(page, `${original.name} ID: ${original.meta.id}`).press(
            "Enter",
        );
        await page.getByLabel(/^Master password/).fill(password);
        await button(page, "Unlock").press("Enter");
        assert.deepEqual(
            await payload(await download(page), password),
            upgraded,
        );
        assert.deepEqual(
            await payload(
                await fileDownload(page, "Download pre-transition backup"),
                password,
            ),
            original,
        );
        assert.deepEqual(errors, []);
        console.log(
            "PASS: built offline historical v1 fixture restore and legacy-to-typed upgrade, wrong password, nested data/Trash/history, exact retained recovery, reload/reopen; fixture enters via supported recovery UI, not an old binary",
        );
    } finally {
        await context.close();
    }
}
