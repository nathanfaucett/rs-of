import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join, resolve } from "node:path";
import { chromium } from "playwright";
import { checkBackupRestore } from "./check-backup.mjs";

// Only synthetic data in a new test browser. Never connect to a live browser.
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
let serverRequests = 0;
let deployment = "initial";
let failWasm = false;
let manifestVersionMatches = true;
let manifestChecks = 0;
const { version } = JSON.parse(
    await readFile(resolve(root, "_app/version.json"), "utf8"),
);
const server = createServer(async (req, res) => {
    serverRequests++;
    try {
        const path = decodeURIComponent(
            new URL(req.url, "http://localhost").pathname,
        );
        if (path === "/offline-test-sensitive") {
            res.writeHead(200, {
                "Content-Type": "text/plain",
                "Cache-Control": "public, max-age=3600",
            });
            res.end("synthetic-sensitive-response-19");
            return;
        }
        const file = resolve(root, `.${path === "/" ? "/index.html" : path}`);
        if (!file.startsWith(`${root}/`)) throw new Error("Invalid path");
        let data = await readFile(file);
        if (failWasm && file.endsWith(".wasm")) {
            res.writeHead(503);
            res.end();
            return;
        }
        if (file.endsWith("version.json")) {
            const matches = manifestVersionMatches || manifestChecks++ === 0;
            data = Buffer.from(
                JSON.stringify({
                    version: `${version}-${deployment}${matches ? "" : "-mismatch"}`,
                }),
            );
        }
        if (file.endsWith("service-worker.js")) {
            const original = data.toString();
            assert.ok(
                original.includes(version),
                "Built worker must include the SvelteKit deployment version",
            );
            data = Buffer.from(
                original.replaceAll(version, `${version}-${deployment}`),
            );
        }
        if (file.endsWith("index.html"))
            data = Buffer.from(
                data
                    .toString()
                    .replace(
                        "</head>",
                        `<meta name="offline-test-deployment:${deployment}" content="synthetic"></head>`,
                    ),
            );
        res.writeHead(200, {
            "Content-Type": types[extname(file)] ?? "application/octet-stream",
            "Cache-Control": "no-store",
        });
        res.end(data);
    } catch {
        res.writeHead(404);
        res.end();
    }
});
let browser;
let context;
const profile = await mkdtemp(join(tmpdir(), "password-manager-offline-"));
try {
    await new Promise((done) => server.listen(0, "127.0.0.1", done));
    const port = server.address().port;
    const origin = `http://127.0.0.1:${port}`;
    const stopServer = () => new Promise((done) => server.close(done));
    const startServer = () =>
        new Promise((done) => server.listen(port, "127.0.0.1", done));
    browser = await chromium.launch({ headless: true });
    context = await chromium.launchPersistentContext(profile, {
        headless: true,
    });
    const errors = [];
    context.on("weberror", (error) => errors.push(error.error().message));
    const page = await context.newPage();
    page.setDefaultTimeout(15000);
    await page.goto(origin);
    await page
        .getByText("Offline ready", { exact: true })
        .waitFor()
        .catch(async (error) => {
            console.error(await page.locator("body").innerText());
            throw error;
        });
    await context.setOffline(true);
    await stopServer();
    const before = serverRequests;
    await page.reload();
    await page
        .getByRole("button", { name: "Create new vault", exact: true })
        .click();
    await page.getByLabel(/^Vault name/).fill("offline-test");
    await page.getByLabel(/^Master password/).fill("synthetic-master-19");
    await page.getByLabel(/^Confirm password/).fill("synthetic-master-19");
    await page
        .getByRole("button", { name: "Create vault", exact: true })
        .click();
    await page.getByRole("button", { name: "New secret", exact: true }).click();
    await page.getByLabel(/^Name/).fill("Synthetic offline Login");
    await page
        .getByLabel("Username", { exact: true })
        .fill("synthetic-user-19");
    await page.getByLabel(/^Password/).fill("synthetic-secret-19");
    await page.getByLabel(/^Confirm password/).fill("synthetic-secret-19");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await page
        .getByRole("button", { name: "Synthetic offline Login", exact: true })
        .click();
    await page.getByLabel("Username", { exact: true }).fill("edited-user-19");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await page.getByRole("button", { name: "Lock vault", exact: true }).click();
    const vaultId = await page
        .getByRole("button", { name: /^offline-test ID: / })
        .locator("span")
        .nth(1)
        .innerText();
    await page.reload();
    await page.close();
    const reopened = await context.newPage();
    await reopened.goto(origin);
    await reopened
        .getByRole("button", { name: `offline-test ${vaultId}`, exact: true })
        .click();
    await reopened.getByLabel(/^Master password/).fill("synthetic-master-19");
    await reopened.getByRole("button", { name: "Unlock", exact: true }).click();
    await reopened
        .getByRole("button", { name: "Synthetic offline Login", exact: true })
        .click();
    assert.equal(
        await reopened.getByLabel("Username", { exact: true }).inputValue(),
        "edited-user-19",
    );
    assert.equal(
        await reopened.getByLabel(/^Password/).inputValue(),
        "synthetic-secret-19",
    );
    assert.equal(
        serverRequests,
        before,
        "Offline flow must not reach the server",
    );
    console.log(
        "PASS: built app offline reload, create, edit, lock, close, start, select, unlock and persisted Login; zero server requests",
    );
    await reopened.getByRole("button", { name: "Cancel", exact: true }).click();
    await reopened
        .getByRole("button", { name: "Lock vault", exact: true })
        .click();
    const initialCaches = await reopened.evaluate(() => caches.keys());
    assert.equal(
        initialCaches.filter((name) => name.startsWith("password-manager-app-"))
            .length,
        1,
    );
    const cacheContents = await reopened.evaluate(async () => {
        const names = await caches.keys();
        const cache = await caches.open(
            names.find((name) => name.startsWith("password-manager-app-")),
        );
        const requests = await cache.keys();
        return Promise.all(
            requests.map(async (request) => ({
                url: request.url,
                body: request.url.endsWith(".wasm")
                    ? ""
                    : await (await cache.match(request)).text(),
            })),
        );
    });
    assert.ok(
        cacheContents.some((entry) => entry.url.endsWith(".wasm")),
        "Database WASM must be precached",
    );
    assert.ok(
        cacheContents.every(
            (entry) =>
                !/synthetic-master-19|synthetic-secret-19|edited-user-19/.test(
                    entry.body,
                ),
        ),
        "App cache must not contain vault content or passwords",
    );
    await reopened.evaluate(async () => {
        await (await caches.open("other-application-cache")).put(
            "/unrelated",
            new Response("keep me"),
        );
    });
    await startServer();
    await context.setOffline(false);
    await reopened.evaluate(async () => {
        await fetch("/offline-test-sensitive");
        await fetch("/offline-test-sensitive", {
            method: "POST",
            body: "synthetic-sensitive-request-19",
        });
    });
    assert.equal(
        await reopened.evaluate(
            async () => !!(await caches.match("/offline-test-sensitive")),
        ),
        false,
    );
    console.log(
        "PASS: sensitive GET and POST responses outside the asset allowlist are not cached",
    );
    deployment = "failed";
    failWasm = true;
    const update = async (expected) => {
        assert.equal(
            await reopened.evaluate(async () => {
                const registration =
                    await navigator.serviceWorker.getRegistration();
                await registration.update();
                const worker = registration.installing;
                if (!worker) throw new Error("Expected a new worker install");
                return new Promise((resolve, reject) => {
                    const timeout = setTimeout(
                        () => reject(new Error("Update timed out")),
                        20000,
                    );
                    const changed = () => {
                        if (
                            worker.state === "installed" ||
                            worker.state === "redundant"
                        ) {
                            clearTimeout(timeout);
                            worker.removeEventListener("statechange", changed);
                            resolve(worker.state);
                        }
                    };
                    worker.addEventListener("statechange", changed);
                    changed();
                });
            }),
            expected,
        );
    };
    await update("redundant");
    failWasm = false;
    deployment = "raced";
    manifestVersionMatches = false;
    await update("redundant");
    manifestVersionMatches = true;
    console.log(
        "PASS: mismatched deployment version rejects installation without replacing the old cache",
    );
    assert.deepEqual(await reopened.evaluate(() => caches.keys()), [
        ...initialCaches,
        "other-application-cache",
    ]);
    await context.setOffline(true);
    await reopened.reload();
    await reopened.getByText("Offline ready", { exact: true }).waitFor();
    assert.ok(
        (await reopened.content()).includes("offline-test-deployment:initial"),
    );
    console.log(
        "PASS: failed WASM update leaves the complete old app usable offline; unrelated cache retained",
    );
    await context.setOffline(false);
    deployment = "next";
    failWasm = false;
    await update("installed");
    await reopened.getByRole("button", { name: "Check offline setup" }).click();
    await reopened.getByText(/Offline update ready/).waitFor();
    assert.ok(
        (await reopened.content()).includes("offline-test-deployment:initial"),
    );
    await reopened.close();
    let next = await context.newPage();
    await next.goto(origin);
    await next.getByText("Offline ready", { exact: true }).waitFor();
    assert.ok((await next.content()).includes("offline-test-deployment:next"));
    const nextCaches = await next.evaluate(() => caches.keys());
    assert.equal(
        nextCaches.filter((name) => name.startsWith("password-manager-app-"))
            .length,
        1,
    );
    assert.ok(nextCaches.includes("other-application-cache"));
    await context.setOffline(true);
    await next.reload();
    await next
        .getByRole("button", { name: `offline-test ${vaultId}`, exact: true })
        .click();
    await next.getByLabel(/^Master password/).fill("synthetic-master-19");
    await next.getByRole("button", { name: "Unlock", exact: true }).click();
    await next
        .getByRole("button", { name: "Synthetic offline Login", exact: true })
        .click();
    assert.equal(
        await next.getByLabel("Username", { exact: true }).inputValue(),
        "edited-user-19",
    );
    console.log(
        "PASS: successful update waits for old tabs to close, replaces only app caches, and preserves the source vault",
    );
    await context.close();
    await stopServer();
    const coldRequests = serverRequests;
    context = await chromium.launchPersistentContext(profile, {
        headless: true,
        offline: true,
    });
    next = context.pages()[0];
    await next.goto(origin);
    await next
        .getByRole("button", { name: `offline-test ${vaultId}`, exact: true })
        .click();
    await next.getByLabel(/^Master password/).fill("synthetic-master-19");
    await next.getByRole("button", { name: "Unlock", exact: true }).click();
    await next
        .getByRole("button", { name: "Synthetic offline Login", exact: true })
        .click();
    assert.equal(
        await next.getByLabel("Username", { exact: true }).inputValue(),
        "edited-user-19",
    );
    assert.equal(serverRequests, coldRequests);
    console.log(
        "PASS: new browser process starts offline and reopens the saved vault with the server stopped",
    );
    await startServer();
    assert.deepEqual(
        errors,
        [],
        "Core flow must not produce unhandled browser errors",
    );
    await checkBackupRestore(browser, origin);
    await next.evaluate(async () => {
        const names = await caches.keys();
        const cache = await caches.open(
            names.find((name) => name.startsWith("password-manager-app-")),
        );
        const wasm = (await cache.keys()).find((request) =>
            request.url.endsWith(".wasm"),
        );
        await cache.delete(wasm);
    });
    await next.getByRole("button", { name: "Check offline setup" }).click();
    await next.getByText(/Offline assets are missing/).waitFor();
    assert.equal(
        await next.evaluate(async () => {
            const names = await caches.keys();
            const cache = await caches.open(
                names.find((name) => name.startsWith("password-manager-app-")),
            );
            const wasm = (await cache.keys()).find((request) =>
                request.url.endsWith(".wasm"),
            );
            return !!wasm;
        }),
        false,
    );
    console.log(
        "PASS: evicted WASM reports missing offline assets, not offline readiness",
    );
    await context.close();

    failWasm = true;
    const failed = await browser.newContext();
    const failedPage = await failed.newPage();
    await failedPage.goto(origin);
    await failedPage.getByText(/Offline setup failed/).waitFor();
    assert.deepEqual(await failedPage.evaluate(() => caches.keys()), []);
    console.log(
        "PASS: first-install WASM failure is actionable and leaves no partial app cache",
    );
    await failed.close();
    failWasm = false;

    for (const fault of [
        "SecurityError",
        "QuotaExceededError",
        "unsupported",
    ]) {
        const isolated = await browser.newContext();
        await isolated.addInitScript((fault) => {
            if (fault === "unsupported")
                Object.defineProperty(globalThis, "WebAssembly", {
                    value: undefined,
                });
            else if (fault === "QuotaExceededError") {
                const transaction = IDBDatabase.prototype.transaction;
                IDBDatabase.prototype.transaction = function (...args) {
                    if (this.name === "password-manager-offline-probe")
                        throw new DOMException(
                            "Synthetic write quota fault",
                            fault,
                        );
                    return transaction.apply(this, args);
                };
            } else
                IDBFactory.prototype.databases = () =>
                    Promise.reject(
                        new DOMException("Synthetic storage fault", fault),
                    );
        }, fault);
        const faultPage = await isolated.newPage();
        await faultPage.goto(origin);
        const message =
            fault === "SecurityError"
                ? /Local storage is denied/
                : fault === "QuotaExceededError"
                  ? /Storage is full/
                  : /Use a current browser with HTTPS/;
        await faultPage
            .getByRole("alert")
            .filter({ hasText: message })
            .waitFor();
        await faultPage.getByText(/Local storage is not a backup/).waitFor();
        console.log(
            `PASS: actionable ${fault} state (injected browser API fault)`,
        );
        await isolated.close();
    }

    const fresh = await browser.newContext({ offline: true });
    const freshPage = await fresh.newPage();
    await assert.rejects(freshPage.goto(origin), /ERR_INTERNET_DISCONNECTED/);
    console.log("PASS: first-ever offline visit is not claimed as supported");
    await fresh.close();
} finally {
    await context?.close();
    await browser?.close();
    await new Promise((done) => server.close(done));
    await rm(profile, { recursive: true, force: true });
}
