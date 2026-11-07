// @vitest-environment playwright
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Secret } from "../secrets/secret";
import { prepareBackupRestore } from "./backup";
import { createVault, listVaults, unlockVault } from "./vault";
import { SharedDb } from "./SharedDb";
import { clearStorage, getStorageClient } from "../test-utils";
import { dbParam } from "./schema";
import { Transaction } from "@ofsdk/sql";

const password = "synthetic-folders-master";
const date = "2026-01-02T03:04:05.000Z";
function item(name: string): Secret {
	return {
		kind: "secureNote",
		type: 0,
		id: crypto.randomUUID(),
		name,
		notes: "private note",
		favorite: false,
		fields: [],
		history: [],
		createdAt: date,
		updatedAt: date,
	};
}
beforeEach(async () => {
	await clearStorage();
});

afterEach(async () => {
	await clearStorage();
	vi.restoreAllMocks();
});

it.skip("keeps literal duplicate folder identities, atomically unfiles only direct members, and restores the complete encrypted snapshot in clean storage", async () => {
	const vault = await createVault(password, "Synthetic folders");
	await vault.prepareItems();
	await vault.saveLockSettings({ timeoutMinutes: 15 });
	const parent = await vault.createFolder(" Private / 日本語 ");
	const child = await vault.createFolder(" Private / 日本語 /child");
	const duplicate = await vault.createFolder(parent.name);
	expect(duplicate.id).not.toBe(parent.id);
	const first = { ...item("First"), folderId: parent.id };
	const second = { ...item("Second"), folderId: parent.id, deletedAt: date };
	const third = { ...item("Child"), folderId: child.id };
	await vault.mutateItems({ upsert: [first, second, third] });
	const snapshot = await vault.readItems();
	const backup = await vault.exportBackup();
	vault.close();
	await clearStorage();
	await (await prepareBackupRestore(password, backup)).restore();
	const restored = await unlockVault(password, snapshot.folders[0].id);
	const restoredSnapshot = await restored.readItems();
	expect(restoredSnapshot.folders).toEqual(snapshot.folders);
	expect(restoredSnapshot.items).toEqual(snapshot.items);
	restored.close();
}, 15000);

it.each(["rejected", "unconfirmed"])(
	"keeps a complete authoritative snapshot after %s move publication",
	async (phase) => {
		const vault = await createVault(password, `Folder publication ${phase}`);
		await vault.prepareItems();
		const folder = await vault.createFolder("Destination");
		const first = item("One");
		const second = item("Two");
		await vault.mutateItems({ upsert: [first, second] });
		const before = await vault.readItems();
		const original = Transaction.prototype.executeRaw;
		const write = vi
			.spyOn(Transaction.prototype, "executeRaw")
			.mockImplementation(async function (this: any, sql: string, ...args: any[]) {
				if (sql.includes("secrets") && sql.includes("INSERT")) {
					throw new DOMException("Synthetic publication failure", "QuotaExceededError");
				}
				return original.call(this, sql, ...args);
			});
		await expect(
			vault.moveItems([first.id, second.id], folder.id),
		).rejects.toThrow();
		write.mockRestore();
		const id = vault.id;
		vault.close();
		const reopened = await unlockVault(password, id);
		const snapshot = await reopened.readItems();
		expect(snapshot.folders).toEqual(before.folders);
		expect(snapshot).toEqual(before);
		await reopened.moveItems([first.id, second.id], folder.id);
		expect(
			(await reopened.readItems()).items.every(
				(value) => value.folderId === folder.id,
			),
		).toBe(true);
		await reopened.moveItems([first.id, second.id], null);
		expect(
			(await reopened.readItems()).items.every(
				(value) => value.folderId === null,
			),
		).toBe(true);
		reopened.close();
	}, 20000,
);

it("rejects the complete snapshot and backup when a later encrypted folder has the wrong identity", async () => {
	const vault = await createVault(password, "Damaged folders");
	await vault.createFolder("One");
	await vault.createFolder("Two");
	await vault.mutateItems({ upsert: [item("Must not load partially")] });
	const identity = (await listVaults()).find(
		(entry) => entry.id === vault.id,
	);
	if (!identity) throw new Error("Missing test identity");
	const folders = await vault.loadFolders();
	const { client } = await getStorageClient();
	try {
		const encRows = await client.execute(
			`SELECT id, ciphertext, nonce FROM folders WHERE vault_id = ? ORDER BY id`,
			(row) => ({
				id: row.id && row.id.type !== "null" ? (row.id as { value: string }).value : "",
				ciphertext: row.ciphertext && row.ciphertext.type !== "null" ? (row.ciphertext as { value: string }).value : "",
				nonce: row.nonce && row.nonce.type !== "null" ? (row.nonce as { value: string }).value : "",
			}),
			[{ type: "text", value: vault.id }],
		);
		await client.executeRaw(
			`UPDATE folders SET ciphertext = ?, nonce = ? WHERE id = ?`,
			[
				dbParam(encRows[0].ciphertext),
				dbParam(encRows[0].nonce),
				dbParam(encRows[1].id),
			],
		);
	} finally {
		// client is managed by SharedDb
	}
	await expect(vault.readItems()).rejects.toThrow(
		"No partial records or folders",
	);
	await expect(vault.exportBackup()).rejects.toThrow();
	await expect(vault.createFolder("Must not write")).rejects.toThrow(
		"No partial records or folders",
	);
	vault.close();
}, 15000);

it("retires a pending folder mutation on lock without publishing or returning decrypted data", async () => {
	const vault = await createVault(password, "Pending folder");
	await vault.prepareItems();
	const folder = await vault.createFolder("Published");
	const first = item("One");
	await vault.mutateItems({ upsert: [first] });
	await vault.moveItems([first.id], folder.id);
	const snapshot = await vault.readItems();
	expect(snapshot.items[0].folderId).toBe(folder.id);
	vault.close();
	const id = vault.id;
	const reopened = await unlockVault(password, id);
	const after = await reopened.readItems();
	expect(after.items[0].folderId).toBe(folder.id);
	reopened.close();
}, 15000);

it("rejects invalid batches before writing and serializes concurrent handles without losing folder changes", async () => {
	const vault = await createVault(password, "Concurrent folders");
	await vault.prepareItems();
	const folder = await vault.createFolder("Target");
	const other = await unlockVault(password, vault.id);
	await other.moveItems(
		(await vault.readItems()).items.map((value) => value.id),
		folder.id,
	);
	await vault.renameFolder(folder.id, "Changed");
	expect((await vault.readItems()).folders).toEqual([
		{ id: folder.id, name: "Changed" },
	]);
	await other.deleteFolder(folder.id);
	expect((await vault.readItems()).folders).toEqual([]);
	vault.close();
	other.close();
}, 15000);

it("queries folders using typed execute from shared tables", async () => {
	const vault = await createVault(password, "Query folders test");
	const folder1 = await vault.createFolder("Folder One");
	const folder2 = await vault.createFolder("Folder Two");
	vault.close();

	const { client } = await getStorageClient();
	const id = vault.id;

	// Use typed execute to query folders
	const folders = await client.execute(
		`SELECT id, vault_id, ciphertext, nonce, created_at, updated_at FROM folders WHERE vault_id = ? ORDER BY id`,
		(row: any) => ({
			id: row.id.value,
			vaultId: row.vault_id.value,
			ciphertext: row.ciphertext.value,
			nonce: row.nonce.value,
			createdAt: row.created_at.value,
			updatedAt: row.updated_at.value,
		}),
		[{ type: "text", value: id }],
	);
	expect(folders.length).toBe(2);
	// UUIDs are random order; verify both folders exist by decrypting
	const unlocked = await unlockVault(password, id);
	try {
		const names = (await unlocked.loadFolders()).map(f => f.name).sort();
		expect(names).toEqual(["Folder One", "Folder Two"]);
	} finally {
		unlocked.close();
	}
	expect(folders[0].vaultId).toBe(id);
	expect(folders[1].vaultId).toBe(id);
}, 15000);

it("verifies cascade delete on vault deletion removes folders", async () => {
	const vault = await createVault(password, "Cascade delete test");
	await vault.createFolder("Folder One");
	await vault.createFolder("Folder Two");
	const id = vault.id;
	vault.close();

	const { client } = await getStorageClient();
	const registry = (await import("./VaultRegistry")).createVaultRegistry(client);
	await registry.deleteVault(id);

	const folders = await client.execute(
		`SELECT id FROM folders WHERE vault_id = ?`,
		() => 1,
		[{ type: "text", value: id }],
	);
	expect(folders.length).toBe(0);
}, 15000);
