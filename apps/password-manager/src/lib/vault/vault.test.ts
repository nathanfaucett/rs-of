// @vitest-environment playwright
import { afterEach, expect, it } from "vitest";
import { createVault, listVaults, unlockVault } from "./vault";
import { clearStorage, getStorageClient } from "../test-utils";
import { SharedDb } from "./SharedDb";

afterEach(async () => {
	await clearStorage();
});

it("authenticates an empty vault after close and reopen", async () => {
	const name = `test-empty-${crypto.randomUUID()}`;
	const vault = await createVault("original-password", name);
	const id = vault.id;
	vault.close();
	await expect(unlockVault("wrong-password", id)).rejects.toThrow(
		"Wrong password",
	);
	const reopened = await unlockVault("original-password", id);
	try {
		expect(await reopened.loadSecrets()).toEqual([]);
	} finally {
		reopened.close();
	}
}, 15000);

it("creates a vault and reopens without repeating schema changes", async () => {
	const vault = await createVault(
		"test-only-master-password",
		`test-prepared-${crypto.randomUUID()}`,
	);
	const id = vault.id;
	vault.close();
	const entry = (await listVaults()).find((v) => v.id === id);
	if (!entry) throw new Error("Missing persisted vault identity");
	const { client } = await getStorageClient();
	try {
		const metaResult = await client.executeRaw(
			`SELECT * FROM vaults WHERE id = ?`,
			[{ type: "text", value: id }],
		);
		expect(metaResult[0]?.rows.length).toBe(1);
		const secretsResult = await client.executeRaw(
			`SELECT * FROM secrets WHERE vault_id = ?`,
			[{ type: "text", value: id }],
		);
		expect(secretsResult[0]?.rows.length).toBe(0);
		const foldersResult = await client.executeRaw(
			`SELECT * FROM folders WHERE vault_id = ?`,
			[{ type: "text", value: id }],
		);
		expect(foldersResult[0]?.rows.length).toBe(0);
	} finally {
	}
	const reopened = await unlockVault("test-only-master-password", id);
	expect(reopened.id).toBe(id);
	reopened.close();
}, 15000);

it("unlocks vault even if schema has extra columns (forward compatible)", async () => {
	const name = `test-forward-compat-${crypto.randomUUID()}`;
	const vault = await createVault("test-only-master-password", name);
	const id = vault.id;
	vault.close();
	const entry = (await listVaults()).find((v) => v.id === id);
	if (!entry) throw new Error("Missing persisted vault identity");
	const { client } = await getStorageClient();
	try {
		await client.executeRaw(
			`ALTER TABLE secrets ADD COLUMN future_note TEXT`,
		);
		const reopened = await unlockVault("test-only-master-password", id);
		expect(reopened.id).toBe(id);
		reopened.close();
		const separate = await createVault("another-password", name);
		expect(separate.id).not.toBe(id);
		separate.close();
	} finally {
	}
}, 15000);

it("persists encrypted logins across lock/unlock and permanently removes a deleted login", async () => {
	const name = `test-crud-${crypto.randomUUID()}`;
	const password = "test-only-master-password";
	const plaintext = JSON.stringify({
		kind: "login",
		id: crypto.randomUUID(),
		name: "Test login",
		username: "test-user",
		secret: "test-password",
		uris: [
			{ uri: "https://example.com", match: "domain" },
			{ uri: "https://example.org", match: "exact" },
		],
		notes: "",
		favorite: false,
		fields: [],
		history: [],
		createdAt: new Date().toISOString(),
		updatedAt: new Date().toISOString(),
	});
	const vault = await createVault(password, name);
	const vaultId = vault.id;
	let id: string;
	try {
		const stored = await vault.encryptAndStore(plaintext);
		id = stored.id;
		const secrets = await vault.loadSecrets();
		expect(secrets.length).toBe(1);
		expect(secrets[0].id).toBe(id);
		expect("secret" in secrets[0] && secrets[0].secret).toBe("test-password");
	} finally {
		vault.close();
	}
	const unlocked = await unlockVault(password, vaultId);
	try {
		expect(
			await unlocked.decryptSecret((await unlocked.loadSecrets())[0]),
		).toBe(plaintext);
		await unlocked.deleteSecret(id);
		expect(await unlocked.loadSecrets()).toEqual([]);
	} finally {
		unlocked.close();
	}
	const reopened = await unlockVault(password, vaultId);
	try {
		expect(await reopened.loadSecrets()).toEqual([]);
	} finally {
		reopened.close();
	}
}, 15000);

it("lists vaults with migrated flag", async () => {
	const vault = await createVault("password", `test-list-${crypto.randomUUID()}`);
	const id = vault.id;
	vault.close();
	const vaults = await listVaults();
	const found = vaults.find((v) => v.id === id);
	expect(found).toBeDefined();
	expect(found!.migrated).toBe(true);
	expect(found!.status).toBe("ready");
	vault.close();
}, 15000);

it("deletes vault and cascades to secrets, folders, lock_settings", async () => {
	const name = `test-delete-${crypto.randomUUID()}`;
	const vault = await createVault("password", name);
	const id = vault.id;
	const plaintext = JSON.stringify({
		kind: "secureNote",
		type: 0,
		id: crypto.randomUUID(),
		name: "Test",
		notes: "secret",
		favorite: false,
		fields: [],
		history: [],
		createdAt: new Date().toISOString(),
		updatedAt: new Date().toISOString(),
	});
	await vault.encryptAndStore(plaintext);
	await vault.createFolder("Test Folder");
	await vault.saveLockSettings({ timeoutMinutes: 15 });
	vault.close();

	const { client } = await getStorageClient();
	const beforeSecrets = await client.executeRaw(
		`SELECT * FROM secrets WHERE vault_id = ?`,
		[{ type: "text", value: id }],
	);
	const beforeFolders = await client.executeRaw(
		`SELECT * FROM folders WHERE vault_id = ?`,
		[{ type: "text", value: id }],
	);
	const beforeLock = await client.executeRaw(
		`SELECT * FROM lock_settings WHERE vault_id = ?`,
		[{ type: "text", value: id }],
	);
	expect(beforeSecrets[0]?.rows.length).toBeGreaterThan(0);
	expect(beforeFolders[0]?.rows.length).toBeGreaterThan(0);
	expect(beforeLock[0]?.rows.length).toBeGreaterThan(0);

	const registry = (await import("./VaultRegistry")).createVaultRegistry(client);
	await registry.deleteVault(id);

	const afterSecrets = await client.executeRaw(
		`SELECT * FROM secrets WHERE vault_id = ?`,
		[{ type: "text", value: id }],
	);
	const afterFolders = await client.executeRaw(
		`SELECT * FROM folders WHERE vault_id = ?`,
		[{ type: "text", value: id }],
	);
	const afterLock = await client.executeRaw(
		`SELECT * FROM lock_settings WHERE vault_id = ?`,
		[{ type: "text", value: id }],
	);
	const afterVault = await client.executeRaw(
		`SELECT * FROM vaults WHERE id = ?`,
		[{ type: "text", value: id }],
	);
	expect(afterSecrets[0]?.rows.length).toBe(0);
	expect(afterFolders[0]?.rows.length).toBe(0);
	expect(afterLock[0]?.rows.length).toBe(0);
	expect(afterVault[0]?.rows.length).toBe(0);
}, 15000);
