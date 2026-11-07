import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { download, logicalPayload, payload } from "./check-backup.mjs";

const button = (page, name) => page.getByRole("button", { name, exact: true });
const password = "synthetic-qualification-20";
const fixtureRoot = new URL(
    "../src/test/fixtures/password-manager/",
    import.meta.url,
);
const expected = JSON.parse(
    await readFile(new URL("export-expected.json", fixtureRoot), "utf8"),
);
const local = JSON.parse(
    await readFile(new URL("export-local.json", fixtureRoot), "utf8"),
);

async function create(page, name) {
    await button(page, "Create new vault").press("Enter");
    await page.getByLabel(/^Vault name/).fill(name);
    await page.getByLabel(/^Master password/).fill(password);
    await page.getByLabel(/^Confirm password/).fill(password);
    await button(page, "Create vault").press("Enter");
    await button(page, "New secret").waitFor();
}

async function preview(page, text) {
    await button(page, "Import Bitwarden JSON").press("Enter");
    await page.getByText(/Plaintext files expose credentials/).waitFor();
    await page.getByLabel("Personal JSON file", { exact: true }).setInputFiles({
        name: "synthetic-only.json",
        mimeType: "application/json",
        buffer: Buffer.from(text),
    });
    await button(page, "Validate import").press("Enter");
    await page
        .getByRole("heading", { name: "Import preview", exact: true })
        .waitFor();
}

async function commit(page) {
    await button(page, "Import all as new").press("Enter");
    await page.getByText(/Import verified/).waitFor();
    await button(page, "New secret").waitFor();
}

async function wire(page) {
    await button(page, "Export Bitwarden JSON").press("Enter");
    await page.getByText(/not encrypted/).waitFor();
    await page
        .getByText(/active items; .*folders; .*Trash items excluded/)
        .waitFor();
    const pending = page.waitForEvent("download");
    await button(page, "Download plaintext JSON").press("Enter");
    const chunks = [];
    for await (const chunk of await (await pending).createReadStream())
        chunks.push(chunk);
    return JSON.parse(Buffer.concat(chunks).toString("utf8"));
}

function logicalWire(value) {
    const result = structuredClone(value);
    const folders = new Map(
        result.folders.map((folder) => [folder.id, folder.name]),
    );
    result.folders = result.folders
        .map(({ name }) => ({ name }))
        .sort((a, b) => a.name.localeCompare(b.name));
    result.items = result.items
        .map(({ id, ...item }) => ({
            ...item,
            folderId: folders.get(item.folderId) ?? null,
        }))
        .sort((a, b) => a.name.localeCompare(b.name));
    return result;
}

export async function checkJsonRoundTrip(page) {
    await create(page, "Complete portability source");
    await preview(page, JSON.stringify(expected));
    for (const kind of ["Login", "Secure Note", "Card", "Identity", "SSH Key"])
        await page
            .getByLabel("Import counts")
            .getByText(`${kind}: 1`, { exact: true })
            .waitFor();
    await commit(page);
    await page
        .getByLabel("Lock after inactivity", { exact: true })
        .selectOption("15");
    await button(page, "Save lock settings").press("Enter");
    await page.getByText("Lock settings saved.", { exact: true }).waitFor();
    await button(page, "New folder").press("Enter");
    await page
        .getByLabel("Folder name", { exact: true })
        .fill("Empty qualification 日本語");
    await button(page, "Save folder").press("Enter");
    await page
        .getByLabel("Folder name", { exact: true })
        .waitFor({ state: "hidden" });
    await button(page, "New secure note").press("Enter");
    await page.getByLabel(/^Name/).fill("EXCLUDED-QUALIFICATION-TRASH");
    await page
        .getByLabel("Notes", { exact: true })
        .fill("PRIVATE-TRASH-20\nRetain in encrypted recovery only");
    await button(page, "Save").press("Enter");
    await page.getByText("Saved.", { exact: true }).waitFor();
    const row = page
        .locator("li")
        .filter({ has: button(page, "EXCLUDED-QUALIFICATION-TRASH") });
    await row
        .getByRole("button", { name: "Delete", exact: true })
        .press("Enter");
    await button(page, "Confirm delete").press("Enter");
    await page.getByText("Moved to Trash.", { exact: true }).waitFor();
    const original = await payload(await download(page), password);
    const exported = await wire(page);
    assert.deepEqual(
        logicalWire(exported),
        logicalWire({
            ...expected,
            folders: [
                ...expected.folders,
                { id: "empty", name: "Empty qualification 日本語" },
            ],
        }),
    );
    assert.equal(JSON.stringify(exported).includes("PRIVATE-TRASH-20"), false);
    assert.deepEqual(
        await payload(await download(page), password),
        original,
        "Export must not change the source",
    );
    await button(page, "Lock vault").press("Enter");

    await create(page, "Complete portability destination");
    const empty = await payload(await download(page), password);
    for (const invalid of [
        "{broken",
        JSON.stringify({
            ...exported,
            unknownQualification: "PRIVATE-UNKNOWN-20",
        }),
        JSON.stringify({
            ...exported,
            items: [
                ...exported.items,
                { ...exported.items[0], id: "unsupported-kind", type: 99 },
            ],
        }),
    ]) {
        await preview(page, invalid);
        await page.getByText("Import blocked", { exact: true }).waitFor();
        assert.equal(await button(page, "Import all as new").count(), 0);
        assert.equal(
            (await page.locator("body").innerText()).includes(
                "PRIVATE-UNKNOWN-20",
            ),
            false,
        );
        await button(page, "Cancel import").press("Enter");
        assert.deepEqual(await payload(await download(page), password), empty);
    }
    await preview(page, JSON.stringify(exported));
    await button(page, "Cancel import").press("Enter");
    assert.deepEqual(
        await payload(await download(page), password),
        empty,
        "Cancelled preview writes nothing",
    );
    await preview(page, JSON.stringify(exported));
    await page.evaluate(() => {
        const put = IDBObjectStore.prototype.put;
        window.qualificationFaults = 0;
        window.restoreQualificationPut = () => {
            IDBObjectStore.prototype.put = put;
        };
        IDBObjectStore.prototype.put = function (...args) {
            if (this.name === "records") {
                window.qualificationFaults++;
                throw new DOMException(
                    "PRIVATE-QUOTA-INJECTION-20",
                    "QuotaExceededError",
                );
            }
            return put.apply(this, args);
        };
    });
    await button(page, "Import all as new").press("Enter");
    await page
        .getByRole("alert")
        .filter({ hasText: /Import could not be confirmed/ })
        .waitFor();
    assert.ok(await page.evaluate(() => window.qualificationFaults > 0));
    await page.evaluate(() => {
        window.restoreQualificationPut();
        delete window.restoreQualificationPut;
        delete window.qualificationFaults;
    });
    assert.equal(
        (await page.content()).includes("PRIVATE-QUOTA-INJECTION-20"),
        false,
    );
    assert.equal(await page.getByText(/Import verified/).count(), 0);
    await button(page, "Cancel import").press("Enter");
    assert.deepEqual(
        await payload(await download(page), password),
        empty,
        "Injected pre-publication quota rejection leaves the complete destination unchanged",
    );
    await preview(page, JSON.stringify(exported));
    await commit(page);
    const imported = await payload(await download(page), password);
    assert.equal(imported.secrets.length, 5);
    assert.equal(imported.folders.length, 3);
    const second = await wire(page);
    assert.deepEqual(logicalWire(second), logicalWire(exported));
    assert.ok(
        second.items.every(
            (item) => !exported.items.some((source) => source.id === item.id),
        ),
    );
    assert.ok(
        second.folders.every(
            (item) => !exported.folders.some((source) => source.id === item.id),
        ),
    );
    await preview(page, JSON.stringify(exported));
    await page
        .getByText(/Source-ID matches: 5. Logical duplicates: 5/)
        .waitFor();
    assert.equal(await button(page, "Import all as new").isDisabled(), true);
    await button(page, "Cancel import").press("Enter");
    assert.deepEqual(
        await payload(await download(page), password),
        imported,
        "Unconfirmed duplicate writes nothing",
    );
    await preview(page, JSON.stringify(exported));
    await page.getByLabel("Import duplicates as new", { exact: true }).check();
    await commit(page);
    const repeated = await wire(page);
    assert.equal(repeated.items.length, 10);
    assert.equal(repeated.folders.length, 6);
    assert.deepEqual(
        logicalWire(repeated).items,
        [...logicalWire(exported).items, ...logicalWire(exported).items].sort(
            (a, b) => a.name.localeCompare(b.name),
        ),
    );
    await button(page, "Lock vault").press("Enter");
    await page.reload();
    await button(
        page,
        `Complete portability source ID: ${original.meta.id}`,
    ).press("Enter");
    await page.getByLabel(/^Master password/).fill(password);
    await button(page, "Unlock").press("Enter");
    assert.deepEqual(await payload(await download(page), password), original);
    await button(page, "★ SYNTHETIC Login café 日本語 🔐").press("Enter");
    await page.evaluate(() =>
        Object.defineProperty(navigator, "clipboard", {
            configurable: true,
            value: {
                writeText: () =>
                    Promise.reject(new Error("PRIVATE-CLIPBOARD-FAULT-20")),
            },
        }),
    );
    await button(page, "Copy Password").press("Enter");
    await page
        .getByRole("alert")
        .filter({ hasText: /Copy failed. Clipboard access/ })
        .waitFor();
    assert.equal(
        (await page.content()).includes("PRIVATE-CLIPBOARD-FAULT-20"),
        false,
    );
    assert.equal(
        (await page.content()).includes("SYNTHETIC-NOT-A-CREDENTIAL"),
        false,
    );
    await page.evaluate(() => {
        delete navigator.clipboard;
    });
    const now = Date.now();
    await page.clock.setFixedTime(new Date(now));
    await button(page, "Edit").press("Enter");
    await page
        .getByLabel("Notes", { exact: true })
        .fill("UNSAVED-EXPIRED-DRAFT-20");
    await page.clock.setFixedTime(new Date(now + 16 * 60000));
    await page.evaluate(() => window.dispatchEvent(new Event("focus")));
    await page
        .getByText(/Vault locked. Unsaved edits were discarded/)
        .waitFor();
    assert.equal(
        (await page.content()).includes("UNSAVED-EXPIRED-DRAFT-20"),
        false,
    );
    assert.equal(await button(page, "Copy Password").count(), 0);
    await button(
        page,
        `Complete portability source ID: ${original.meta.id}`,
    ).press("Enter");
    await page.getByLabel(/^Master password/).fill(password);
    await button(page, "Unlock").press("Enter");
    assert.deepEqual(await payload(await download(page), password), original);
    console.log(
        "PASS: built offline clipboard-denial injection has fixed feedback; simulated wall-clock suspension/resume retires a dirty draft and preserves committed data",
    );
    console.log(
        "PASS: built offline independent complete five-type JSON UI export/preview/import, every supported field, empty folder, history/URI/custom order, Trash exclusion, corrupt/unknown/type rejection, cancel, duplicate rejection/consent, fresh IDs, source preservation and reopen",
    );
    const backup = await download(page);
    const logical = await logicalPayload(backup, password);
    for (const item of local.items) {
        const actual = logical.items.find((value) => value.name === item.name);
        assert.ok(actual);
        for (const [key, value] of Object.entries(item)) {
            if (["id", "folderId", "importSource"].includes(key)) continue;
            if (key === "createdAt" || key === "updatedAt")
                assert.equal(actual[key], new Date(value).toISOString());
            else if (key === "history")
                assert.deepEqual(
                    actual.history,
                    value.map((entry) => ({
                        ...entry,
                        changedAt: new Date(entry.changedAt).toISOString(),
                    })),
                );
            else assert.deepEqual(actual[key], value);
        }
        assert.deepEqual(actual.importSource, {
            format: "personal-json-v1",
            id: item.id,
        });
    }
    assert.equal(logical.items.filter((item) => item.deletedAt).length, 1);
    return { backup, expected: original, password, logical };
}

export async function checkPortableRecovery(page, recovery) {
    assert.equal(
        await page.getByText("Existing vaults", { exact: true }).count(),
        0,
    );
    await button(page, "Restore encrypted backup").press("Enter");
    await page.getByLabel(/^Encrypted backup file/).setInputFiles({
        name: "synthetic-recovery.json",
        mimeType: "application/json",
        buffer: Buffer.from(recovery.backup),
    });
    await page.getByLabel(/^Backup master password/).fill(recovery.password);
    await button(page, "Validate backup").press("Enter");
    await page.getByText(/Validated backup:/).waitFor();
    await button(page, "Restore vault").press("Enter");
    await page.getByText(/Restore verified:/).waitFor();
    await page.reload();
    await button(
        page,
        `${recovery.expected.name} ID: ${recovery.expected.meta.id}`,
    ).press("Enter");
    await page.getByLabel(/^Master password/).fill(recovery.password);
    await button(page, "Unlock").press("Enter");
    assert.equal(
        await page
            .getByLabel("Lock after inactivity", { exact: true })
            .inputValue(),
        "15",
    );
    const restoredBackup = await download(page);
    assert.deepEqual(
        await payload(restoredBackup, recovery.password),
        recovery.expected,
    );
    assert.deepEqual(
        await logicalPayload(restoredBackup, recovery.password),
        recovery.logical,
    );
    assert.deepEqual(
        logicalWire(await wire(page)),
        logicalWire({
            ...expected,
            folders: [
                ...expected.folders,
                { id: "empty", name: "Empty qualification 日本語" },
            ],
        }),
    );
    await button(page, "Trash").press("Enter");
    await page
        .getByText("EXCLUDED-QUALIFICATION-TRASH", { exact: true })
        .waitFor();
    console.log(
        "PASS: complete portability-source encrypted recovery after source-profile disposal, clean offline profile, exact folders/Trash/history/optional records/settings/authentication/ciphertext and independent active logical oracle",
    );
}
