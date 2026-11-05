import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve } from "node:path";
import { chromium } from "playwright";

const root = resolve("build");
const types = {
    ".html": "text/html",
    ".js": "text/javascript",
    ".css": "text/css",
    ".wasm": "application/wasm",
    ".svg": "image/svg+xml",
    ".png": "image/png",
    ".json": "application/json",
};
let requests = 0;
const server = createServer(async (request, response) => {
    requests++;
    try {
        const path = decodeURIComponent(
            new URL(request.url, "http://localhost").pathname,
        );
        const file = resolve(root, `.${path === "/" ? "/index.html" : path}`);
        assert.ok(file.startsWith(`${root}/`));
        response.setHeader(
            "Content-Type",
            types[extname(file)] ?? "application/octet-stream",
        );
        response.end(await readFile(file));
    } catch {
        response.writeHead(404);
        response.end();
    }
});
let browser;
try {
    await new Promise((done) => server.listen(0, "127.0.0.1", done));
    browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({ acceptDownloads: true });
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    const button = (name) => page.getByRole("button", { name, exact: true });
    await page.goto(`http://127.0.0.1:${server.address().port}`);
    await page.getByText("Offline ready", { exact: true }).waitFor();
    await context.setOffline(true);
    await new Promise((done) => server.close(done));
    const before = requests;
    await page.reload();
    await button("Create new vault").press("Enter");
    await page.getByLabel(/^Vault name/).fill("synthetic-json-export");
    await page
        .getByLabel(/^Master password/)
        .fill("synthetic-json-export-password");
    await page
        .getByLabel(/^Confirm password/)
        .fill("synthetic-json-export-password");
    await button("Create vault").press("Enter");
    await button("New folder").press("Enter");
    await page.getByLabel("Folder name", { exact: true }).fill("Empty/日本語");
    await button("Save folder").press("Enter");
    await page
        .getByLabel("Folder name", { exact: true })
        .waitFor({ state: "hidden" });
    const names = [
        "Synthetic Login",
        "Synthetic Note",
        "Synthetic Card",
        "Synthetic Identity",
        "Synthetic SSH",
    ];
    for (const [index, action] of [
        "New secret",
        "New secure note",
        "New card",
        "New identity",
        "New SSH key",
    ].entries()) {
        await button(action).press("Enter");
        await page.getByLabel(/^Name/).fill(names[index]);
        if (index === 1)
            await page
                .getByLabel("Notes", { exact: true })
                .fill("SYNTHETIC ONLY\n日本語 café\n");
        if (index === 4) {
            await button("Show Private key").press("Enter");
            await page
                .getByLabel(/^Private key/)
                .fill("SYNTHETIC KEY\nNot functional\n");
        }
        await button("Save").press("Enter");
        await page.getByText("Saved.", { exact: true }).waitFor();
        await page.getByLabel(/^Name/).waitFor({ state: "hidden" });
    }
    await button("Lock vault").press("Enter");
    await page.reload();
    await page
        .locator("button")
        .filter({
            has: page.locator(".vault-name", {
                hasText: "synthetic-json-export",
            }),
        })
        .press("Enter");
    await page
        .getByLabel(/^Master password/)
        .fill("synthetic-json-export-password");
    await button("Unlock").press("Enter");
    let downloads = 0;
    page.on("download", () => downloads++);
    await button("Export Bitwarden JSON").press("Enter");
    await page
        .getByText("5 active items; 1 folders; 0 Trash items excluded.", {
            exact: true,
        })
        .waitFor();
    assert.equal(
        await button("Cancel export").evaluate(
            (element) => element === document.activeElement,
        ),
        true,
    );
    await button("Cancel export").press("Enter");
    assert.equal(
        await button("Export Bitwarden JSON").evaluate(
            (element) => element === document.activeElement,
        ),
        true,
    );
    assert.equal(downloads, 0);
    await button("Export Bitwarden JSON").press("Enter");
    await page
        .getByText("5 active items; 1 folders; 0 Trash items excluded.", {
            exact: true,
        })
        .waitFor();
    const pending = page.waitForEvent("download");
    await button("Download plaintext JSON").press("Enter");
    const download = await pending;
    const chunks = [];
    for await (const chunk of await download.createReadStream())
        chunks.push(chunk);
    const data = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    assert.equal(data.encrypted, false);
    assert.equal(data.folders[0].name, "Empty/日本語");
    const items = data.items.sort((a, b) => a.type - b.type);
    assert.deepEqual(
        items.map((item) => item.type),
        [1, 2, 3, 4, 5],
    );
    assert.deepEqual(
        items.map((item) => item.name),
        names,
    );
    assert.deepEqual(items[0].login, {
        username: "",
        password: "",
        totp: "",
        uris: [],
        fido2Credentials: [],
    });
    assert.equal(items[1].notes, "SYNTHETIC ONLY\n日本語 café\n");
    assert.deepEqual(items[1].secureNote, { type: 0 });
    assert.deepEqual(items[2].card, {
        cardholderName: "",
        brand: "",
        number: "",
        expMonth: "",
        expYear: "",
        code: "",
    });
    assert.equal(Object.keys(items[3].identity).length, 18);
    assert.deepEqual(items[4].sshKey, {
        privateKey: "SYNTHETIC KEY\nNot functional\n",
        publicKey: "",
        keyFingerprint: "",
    });
    await button("Export Bitwarden JSON").press("Enter");
    await button("Lock vault").press("Enter");
    assert.equal(await button("Download plaintext JSON").count(), 0);
    assert.equal(downloads, 1);
    assert.equal(requests, before);
    assert.deepEqual(errors, []);
    console.log(
        "PASS: built offline five-type JSON export, Unicode/multiline/optional values, empty folder, keyboard cancel/focus, real download, lock, reload, and no network",
    );
} finally {
    await browser?.close();
    if (server.listening) await new Promise((done) => server.close(done));
}
