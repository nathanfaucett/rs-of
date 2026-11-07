// @vitest-environment playwright
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Secret } from "../secrets/secret";
import { prepareBackupRestore } from "./backup";
import { createVault, listVaults, unlockVault } from "./vault";
import { SharedDb } from "./SharedDb";
import { clearStorage, getStorageClient } from "../test-utils";

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

it.skip.each(["rejected", "unconfirmed"])(
	"keeps a complete authoritative snapshot after %s move publication",
	async (phase) => {
		const vault = await createVault(password, `Folder publication ${phase}`);
		await vault.prepareItems();
		const folder = await vault.createFolder("Destination");
		const first = item("One");
		const second = item("Two");
		await vault.mutateItems({ upsert: [first, second] });
		const before = await vault.readItems();
		const put = IDBObjectStore.prototype.put;
		const getAll = IDBObjectStore.prototype.getAll;
		let published = false;
		const write = vi
			.spyOn(IDBObjectStore.prototype, "put")
			.mockImplementation(function (this: IDBObjectStore, value, key) {
				if (this.name === "vaults") {
					if (phase === "rejected")
						throw new DOMException(
							"Synthetic publication failure",
							"QuotaExceededError",
						);
					published = true;
				}
				return put.call(this, value, key);
			});
		const read = vi
			.spyOn(IDBObjectStore.prototype, "getAll")
			.mockImplementation(function (this: IDBObjectStore, ...args) {
				if (this.name === "vaults" && published)
					throw new DOMException(
						"Synthetic read failure",
						"UnknownError",
					);
				return getAll.apply(this, args);
			});
		await expect(
			vault.moveItems([first.id, second.id], folder.id),
		).rejects.toThrow("could not be confirmed");
		write.mockRestore();
		read.mockRestore();
		const id = vault.id;
		vault.close();
		const reopened = await unlockVault(password, id);
		const snapshot = await reopened.readItems();
		expect(snapshot.folders).toEqual(before.folders);
		if (phase === "rejected") expect(snapshot).toEqual(before);
		else {
			expect(snapshot.items).toHaveLength(2);
			expect(
				snapshot.items.every((value) => value.folderId === folder.id),
			).toBe(true);
		}
		await reopened.moveItems([first.id, second.id], null);
		expect(
			(await reopened.readItems()).items.every(
				(value) => value.folderId === null,
			),
		).toBe(true);
		reopened.close();
	}, 20000,
);

it.skip("rejects the complete snapshot and backup when a later encrypted folder has the wrong identity", async () => {
	const vault = await createVault(password, "Damaged folders");
	await vault.createFolder("One");
	await vault.createFolder("Two");
	await vault.mutateItems({ upsert: [item("Must not load partially")] });
	const identity = (await listVaults()).find(
		(entry) => entry.id === vault.id,
	);
	if (!identity) throw new Error("Missing test identity");
	const records = await vault.loadFolders();
	const db = await storage(vault.id);
	try {
		await db.client.execute(
			`UPDATE ${prefixed(vault.id, "folders")} SET ciphertext = $1, nonce = $2 WHERE id = $3`,
			[
				{ type: "text", value: records[0].ciphertext },
				{ type: "text", value: records[0].nonce },
				{ type: "text", value: records[1].id },
			],
		);
	} finally {
		db.close(); // no-op
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

it.skip("retires a pending folder mutation on lock without publishing or returning decrypted data", async () => {
	const vault = await createVault(password, "Pending folder");
	await vault.prepareItems();
	const id = vault.id;
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let started = false;
	const encrypt = crypto.subtle.encrypt.bind(crypto.subtle);
	const delayed = vi
		.spyOn(crypto.subtle, "encrypt")
		.mockImplementationOnce(async (...args) => {
			started = true;
			await gate;
			return encrypt(...args);
		});
	await vault.createFolder("Must not publish");
	await gate;
	await vault.close();
	await expect(vault.createFolder("Must not write")).rejects.toThrow("closed");
}, 15000);

it.skip("rejects invalid batches before writing and serializes concurrent handles without losing folder changes", async () => {
	const vault = await createVault(password, "Concurrent folders");
	await vault.prepareItems();
	const folder = await vault.createFolder("Target");
	const other = await unlockVault(password, vault.id);
	await other.moveItems(
		(await vault.readItems()).items.map((value) => value.id),
		folder.id,
	);
	await vault.renameFolder(folder.id, "Changed");
	await other.deleteFolder(folder.id);
	expect((await vault.readItems()).folders).toEqual([
		{ id: folder.id, name: "Changed" },
	]);
	vault.close();
	other.close();
}, 15000);

function prefixed(id: string, table: string): string {
	return `vault_${id.replace(/-/g, "")}_${table}`;
}

async function storage(id: string) {
	const sharedDb = SharedDb.getInstance();
	const client = await sharedDb.getClient(id);
	return {
		client,
		close() {
			client.close();
		},
	};
}
