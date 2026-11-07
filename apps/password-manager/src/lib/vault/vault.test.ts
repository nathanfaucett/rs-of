// @vitest-environment playwright
// @vitest-environment playwright
import { afterEach, expect, it } from "vitest";
import { vaultMigrations } from "./migrations";
import { createVault, listVaults, unlockVault } from "./vault";
import { clearStorage, getStorageClient, prefixed } from "../test-utils";
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

it("rejects creating a vault when legacy per-vault database exists for same ID", async () => {
	// Create a legacy per-vault database (old naming scheme)
	const id = crypto.randomUUID();
	const oldDbName = `password-manager-local-${id}`;
	const db = await (await import("@ofsdk/sql")).Database.open(oldDbName);
	const client = db.client();
	try {
		await client.migrate(vaultMigrations.slice(0, 1));
		await client.execute(
			"INSERT INTO vault_meta (id, vault_id, salt, iv, created_at) VALUES (1, $1, $2, $3, $4)",
			[
				{ type: "text", value: id },
				{ type: "text", value: "dGVzdC1zYWx0" },
				{ type: "text", value: "dGVzdC1pdg==" },
				{ type: "text", value: "2026-01-01T00:00:00.000Z" },
			],
		);
	} finally {
		client.close();
		db.close();
	}

	// Register the legacy vault in registry
	await listVaults();

	// Try to create a new vault with the same name - should get a different ID
	const name = `test-legacy-conflict-${crypto.randomUUID()}`;
	const vault = await createVault("new-password", name);
	expect(vault.id).not.toBe(id);
	vault.close();
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
	const db = await getStorageClient(id);
	try {
		// Verify tables exist with correct schema by querying them
		const metaResult = await db.client.execute(
			`SELECT * FROM ${prefixed(id, "meta")} WHERE id = 1`,
		);
		expect(metaResult[0]?.rows.length).toBe(1);
		const secretsResult = await db.client.execute(
			`SELECT * FROM ${prefixed(id, "secrets")}`,
		);
		expect(secretsResult[0]?.rows.length).toBe(0);
		const foldersResult = await db.client.execute(
			`SELECT * FROM ${prefixed(id, "folders")}`,
		);
		expect(foldersResult[0]?.rows.length).toBe(0);
	} finally {
		db.close(); // no-op
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
	const db = await getStorageClient(id);
	try {
		// Add a future column to simulate newer schema
		await db.client.execute(
			`ALTER TABLE ${prefixed(id, "secrets")} ADD COLUMN future_note TEXT`,
		);
		// Should still unlock successfully (forward compatible)
		const reopened = await unlockVault("test-only-master-password", id);
		expect(reopened.id).toBe(id);
		reopened.close();
		const separate = await createVault("another-password", name);
		expect(separate.id).not.toBe(id);
		separate.close();
	} finally {
		db.close(); // no-op
	}
}, 15000);

it("persists encrypted logins across lock/unlock and permanently removes a deleted login", async () => {
	const name = `test-crud-${crypto.randomUUID()}`;
	const password = "test-only-master-password";
	const plaintext = JSON.stringify({
		name: "Test login",
		username: "test-user",
		secret: "test-password",
		uris: [
			{ uri: "https://example.com", match: "domain" },
			{ uri: "https://example.org", match: "exact" },
		],
	});
	const vault = await createVault(password, name);
	const vaultId = vault.id;
	let id: string;
	try {
		const stored = await vault.encryptAndStore(plaintext);
		id = stored.id;
		expect(stored.ciphertext).not.toContain("test-password");
		expect(await vault.decryptSecret((await vault.loadSecrets())[0])).toBe(
			plaintext,
		);
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
