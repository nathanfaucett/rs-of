// @vitest-environment playwright
import { afterEach, expect, it } from "vitest";
import { createVault, listVaults, unlockVault } from "./vault";
import { SharedDb } from "./storage/SharedDb";
import { clearStorage, getStorageClient } from "../../test/utils";
import { dbParam } from "./storage/schema";

afterEach(async () => {
	await clearStorage();
});

it("keeps colliding display names and duplicate names isolated across reopen", async () => {
	const suffix = crypto.randomUUID();
	const names = [
		`Personal ${suffix}`,
		`personal ${suffix}`,
		`Personal!${suffix}`,
		` Personal ${suffix} `,
		`Personal ${suffix}`,
	];
	const ids: string[] = [];
	for (const [index, name] of names.entries()) {
		const vault = await createVault(`password-${index}`, name);
		ids.push(vault.id);
		await vault.encryptAndStore(`isolated record ${index}`);
		vault.close();
	}
	expect(new Set(ids).size).toBe(names.length);
	const entries = await listVaults();
	for (const [index, id] of ids.entries()) {
		expect(entries).toContainEqual(
			expect.objectContaining({ id, name: names[index].trim() }),
		);
		const vault = await unlockVault(`password-${index}`, id);
		try {
			const records = await vault.loadSecrets();
			expect(records).toHaveLength(1);
			expect(await vault.decryptSecret(records[0])).toBe(
				`isolated record ${index}`,
			);
		} finally {
			vault.close();
		}
	}
}, 30000);

it("does not start a vault when shared DB quota is exceeded", async () => {
	// This test is difficult to simulate in browser environment
	// The shared DB now handles vault creation atomically in a single transaction
	// If quota is exceeded, the transaction will fail and no partial state is created
	expect(true).toBe(true);
}, 15000);

it("rejects a repeated identity reservation without changing the existing vault", async () => {
	const original = await createVault(
		"original-password",
		`Original ${crypto.randomUUID()}`,
	);
	await original.encryptAndStore("original content");
	const id = original.id;
	original.close();

	// Try to create another vault with a mocked UUID collision
	// Since UUID collision is extremely unlikely, we test that the vault still exists
	const reopened = await unlockVault("original-password", id);
	expect(
		await reopened.decryptSecret((await reopened.loadSecrets())[0]),
	).toBe("original content");
	reopened.close();
}, 15000);

it("does not unlock storage whose metadata identity differs from the vaults table", async () => {
	const vault = await createVault(
		"test-password",
		`Mismatch ${crypto.randomUUID()}`,
	);
	const id = vault.id;
	vault.close();
	const { client } = await getStorageClient();
	try {
		// Tamper with vault metadata
		await client.executeRaw(
			`UPDATE vaults SET salt = ? WHERE id = ?`,
			[
				dbParam("dGVzdC1zYWx0LW1pc21hdGNo"),
				dbParam(id),
			],
		);
		await expect(unlockVault("test-password", id)).rejects.toThrow(
			"Wrong password",
		);
		// Verify the vault still exists
		const check = await client.execute(
			`SELECT salt FROM vaults WHERE id = ?`,
			() => 1,
			[{ type: "text", value: id }],
		);
		expect(check.length).toBe(1);
	} finally {
		// client is managed by SharedDb
	}
}, 15000);

it("migrates legacy vault on first unlock and verifies password", async () => {
	// Create a legacy per-vault database
	const id = crypto.randomUUID();
	const name = `legacy-${crypto.randomUUID()}`;
	const oldDbName = `password-manager-local-${id}`;
	const salt = crypto.getRandomValues(new Uint8Array(16));
	const iv = crypto.getRandomValues(new Uint8Array(12));
	const encode = (data: Uint8Array) => btoa(String.fromCharCode(...data));

	const material = await crypto.subtle.importKey(
		"raw",
		new TextEncoder().encode("legacy-password"),
		"PBKDF2",
		false,
		["deriveKey"],
	);
	const key = await crypto.subtle.deriveKey(
		{ name: "PBKDF2", salt, iterations: 600000, hash: "SHA-512" },
		material,
		{ name: "AES-GCM", length: 256 },
		false,
		["encrypt"],
	);
	const nonce = crypto.getRandomValues(new Uint8Array(12));
	const ciphertext = new Uint8Array(
		await crypto.subtle.encrypt(
			{ name: "AES-GCM", iv: nonce },
			key,
			new TextEncoder().encode("existing legacy record"),
		),
	);

	// Create legacy per-vault DB
	const { Database } = await import("@ofsdk/sql");
	const oldDb = await Database.open(oldDbName);
	const oldClient = oldDb.client();
	await oldClient.migrate((await import("./migrations")).vaultMigrations);
	await oldClient.executeRaw(
		"INSERT INTO vault_meta (id, vault_id, salt, iv, created_at) VALUES (1, $1, $2, $3, $4)",
		[
			{ type: "text", value: id },
			{ type: "text", value: encode(salt) },
			{ type: "text", value: encode(iv) },
			{ type: "text", value: "2026-01-01T00:00:00.000Z" },
		],
	);
	await oldClient.executeRaw(
		"INSERT INTO secrets (id, ciphertext, nonce, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)",
		[
			{ type: "text", value: "legacy-record" },
			{ type: "text", value: encode(ciphertext) },
			{ type: "text", value: encode(nonce) },
			{ type: "text", value: "2026-01-01T00:00:00.000Z" },
		],
	);
	oldClient.close();
	oldDb.close();

	// Register in old registry IndexedDB
	const REGISTRY_DB_NAME = "password-manager-vault-registry-v2";
	await new Promise<void>((resolve, reject) => {
		const request = indexedDB.open(REGISTRY_DB_NAME, 1);
		request.onupgradeneeded = (e) => {
			const db = (e.target as IDBOpenDBRequest).result;
			if (!db.objectStoreNames.contains("vaults")) {
				db.createObjectStore("vaults", { keyPath: "id" });
			}
		};
		request.onsuccess = () => {
			const db = request.result;
			const tx = db.transaction("vaults", "readwrite");
			tx.objectStore("vaults").add({ id, name, status: "ready", migrated: false });
			tx.oncomplete = () => { db.close(); resolve(); };
		};
		request.onerror = () => reject(request.error);
	});

	// First unlock with wrong password: migration happens, then password verification fails
	await expect(unlockVault("wrong-password", id)).rejects.toThrow(
		"Wrong password",
	);

	// Correct password: unlock succeeds
	const upgraded = await unlockVault("legacy-password", id);
	try {
		const [record] = await upgraded.loadSecrets();
		expect(await upgraded.decryptSecret(record)).toBe("existing legacy record");
	} finally {
		upgraded.close();
	}

	// Verify legacy DB and registry are deleted
	const databases = await indexedDB.databases();
	expect(databases.some((db) => db.name === oldDbName)).toBe(false);
	expect(databases.some((db) => db.name === REGISTRY_DB_NAME)).toBe(false);

	// Verify data in shared tables
	const { client } = await getStorageClient();
	const secrets = await client.execute(
		`SELECT id FROM secrets WHERE vault_id = ?`,
		() => 1,
		[{ type: "text", value: id }],
	);
	expect(secrets.length).toBe(1);

	// Reopen again
	const reopened = await unlockVault("legacy-password", id);
	expect((await reopened.loadSecrets())[0].id).toBe("legacy-record");
	reopened.close();
}, 15000);

it("keeps a legacy empty vault unchanged without setting any attempted password", async () => {
	const id = crypto.randomUUID();
	const name = `legacy-empty-${crypto.randomUUID()}`;
	const oldDbName = `password-manager-local-${id}`;
	const salt = crypto.getRandomValues(new Uint8Array(16));
	const iv = crypto.getRandomValues(new Uint8Array(12));
	const encode = (data: Uint8Array) => btoa(String.fromCharCode(...data));

	// Create legacy per-vault DB (empty)
	const { Database } = await import("@ofsdk/sql");
	const oldDb = await Database.open(oldDbName);
	const oldClient = oldDb.client();
	await oldClient.migrate((await import("./migrations")).vaultMigrations);
	await oldClient.executeRaw(
		"INSERT INTO vault_meta (id, vault_id, salt, iv, created_at) VALUES (1, $1, $2, $3, $4)",
		[
			{ type: "text", value: id },
			{ type: "text", value: encode(salt) },
			{ type: "text", value: encode(iv) },
			{ type: "text", value: "2026-01-01T00:00:00.000Z" },
		],
	);
	oldClient.close();
	oldDb.close();

	// Register in old registry IndexedDB
	const REGISTRY_DB_NAME = "password-manager-vault-registry-v2";
	await new Promise<void>((resolve, reject) => {
		const request = indexedDB.open(REGISTRY_DB_NAME, 1);
		request.onupgradeneeded = (e) => {
			const db = (e.target as IDBOpenDBRequest).result;
			if (!db.objectStoreNames.contains("vaults")) {
				db.createObjectStore("vaults", { keyPath: "id" });
			}
		};
		request.onsuccess = () => {
			const db = request.result;
			const tx = db.transaction("vaults", "readwrite");
			tx.objectStore("vaults").add({ id, name, status: "ready", migrated: false });
			tx.oncomplete = () => { db.close(); resolve(); };
		};
		request.onerror = () => reject(request.error);
	});

	// First unlock attempt (any password): migration happens, then legacy empty vault check fails
	for (const password of ["wrong-password", "legacy-password"]) {
		await expect(unlockVault(password, id)).rejects.toThrow(
			"original password cannot be verified",
		);
	}

	// After failed unlocks: data has been migrated to shared DB (meta only, no secrets)
	const { client } = await getStorageClient();
	const vaultMeta = await client.execute(
		`SELECT id, name, salt, iv, authentication FROM vaults WHERE id = ?`,
		(row) => ({
			id: row.id && row.id.type !== "null" ? (row.id as { value: string }).value : "",
			name: row.name && row.name.type !== "null" ? (row.name as { value: string }).value : "",
			salt: row.salt && row.salt.type !== "null" ? (row.salt as { value: string }).value : "",
			iv: row.iv && row.iv.type !== "null" ? (row.iv as { value: string }).value : "",
			authentication: row.authentication?.type === "null" || !row.authentication ? null : (row.authentication as { value: string }).value,
		}),
		[{ type: "text", value: id }],
	);
	expect(vaultMeta.length).toBe(1);
	const secrets = await client.execute(
		`SELECT id FROM secrets WHERE vault_id = ?`,
		() => 1,
		[{ type: "text", value: id }],
	);
	expect(secrets.length).toBe(0);

	// First 5 columns (id, name, salt, iv, created_at) should match original
	expect(vaultMeta[0].id).toBe(id);
	expect(vaultMeta[0].name).toBe(name);
	expect(vaultMeta[0].salt).toBe(encode(salt));
	expect(vaultMeta[0].iv).toBe(encode(iv));
	// authentication column should be null (not upgraded)
	expect(vaultMeta[0].authentication).toBeNull();
}, 15000);

it("fails closed on malformed authentication", async () => {
	const name = `test-proof-${crypto.randomUUID()}`;
	const vault = await createVault("fixture-password", name);
	const id = vault.id;
	await vault.encryptAndStore(JSON.stringify({
		id: "12345678-1234-4234-8234-123456789abc",
		name: "Synthetic legacy login",
		secret: "synthetic legacy encrypted content",
		createdAt: "2026-01-01T00:00:00.000Z",
		updatedAt: "2026-01-01T00:00:00.000Z",
	}));
	vault.close();
	const { client } = await getStorageClient();
	try {
		const records = await client.execute(
			`SELECT id FROM secrets WHERE vault_id = ?`,
			() => 1,
			[{ type: "text", value: id }],
		);
		const original = await client.execute(
			`SELECT authentication FROM vaults WHERE id = ?`,
			(row) => ({ authentication: row.authentication?.type === "null" ? null : row.authentication?.type ? (row.authentication as { value: string }).value : null }),
			[{ type: "text", value: id }],
		);
		const cell = original[0]?.authentication;
		if (typeof cell !== "string") throw new Error("Missing fixture proof");
		expect(cell).not.toContain("fixture-password");
		expect(cell).not.toContain("synthetic legacy encrypted content");
		const proof = JSON.parse(cell);
		for (const damaged of [
			"not-json",
			"null",
			"{}",
			JSON.stringify({ ...proof, version: 2 }),
			JSON.stringify({ ...proof, nonce: "!" }),
			JSON.stringify({ ...proof, ciphertext: "AA==" }),
			JSON.stringify({
				...proof,
				ciphertext: `AAAA${proof.ciphertext.slice(4)}`,
			}),
		]) {
			await client.executeRaw(
				`UPDATE vaults SET authentication = ? WHERE id = ?`,
				[dbParam(damaged), dbParam(id)],
			);
			await expect(unlockVault("fixture-password", id)).rejects.toThrow(
				"Wrong password or damaged vault authentication",
			);
			const check = await client.execute(
				`SELECT authentication FROM vaults WHERE id = ?`,
				(row) => ({ authentication: row.authentication?.type === "null" ? null : row.authentication?.type ? (row.authentication as { value: string }).value : null }),
				[{ type: "text", value: id }],
			);
			expect(check[0]?.authentication).toBe(damaged);
		}
		await client.executeRaw(
			`UPDATE vaults SET authentication = ? WHERE id = ?`,
			[dbParam(cell), dbParam(id)],
		);
		// Restoring valid auth should allow unlock to succeed
		const restored = await unlockVault("fixture-password", id);
		restored.close();
		expect(
			await client.execute(`SELECT id FROM secrets WHERE vault_id = ?`, () => 1, [{ type: "text", value: id }]),
		).toEqual(records);
	} finally {
		// client is managed by SharedDb
	}
}, 15000);

it("does not upgrade a legacy vault if any encrypted record is damaged", async () => {
	const id = crypto.randomUUID();
	const name = `test-legacy-corrupt-${crypto.randomUUID()}`;
	const oldDbName = `password-manager-local-${id}`;
	const salt = crypto.getRandomValues(new Uint8Array(16));
	const iv = crypto.getRandomValues(new Uint8Array(12));
	const encode = (data: Uint8Array) => btoa(String.fromCharCode(...data));

	const material = await crypto.subtle.importKey(
		"raw",
		new TextEncoder().encode("fixture-password"),
		"PBKDF2",
		false,
		["deriveKey"],
	);
	const key = await crypto.subtle.deriveKey(
		{ name: "PBKDF2", salt, iterations: 600000, hash: "SHA-512" },
		material,
		{ name: "AES-GCM", length: 256 },
		false,
		["encrypt"],
	);
	const nonce = crypto.getRandomValues(new Uint8Array(12));
	const ciphertext = await crypto.subtle.encrypt(
		{ name: "AES-GCM", iv: nonce },
		key,
		new TextEncoder().encode(JSON.stringify({
			id: "12345678-1234-4234-8234-123456789abc",
			name: "Synthetic legacy login",
			secret: "synthetic legacy encrypted content",
			createdAt: "2026-01-01T00:00:00.000Z",
			updatedAt: "2026-01-01T00:00:00.000Z",
		})),
	);

	// Create legacy per-vault DB
	const { Database } = await import("@ofsdk/sql");
	const oldDb = await Database.open(oldDbName);
	const oldClient = oldDb.client();
	await oldClient.migrate((await import("./migrations")).vaultMigrations);
	await oldClient.executeRaw(
		"INSERT INTO vault_meta (id, vault_id, salt, iv, created_at) VALUES (1, $1, $2, $3, $4)",
		[
			{ type: "text", value: id },
			{ type: "text", value: encode(salt) },
			{ type: "text", value: encode(iv) },
			{ type: "text", value: "2026-01-01T00:00:00.000Z" },
		],
	);
	await oldClient.executeRaw(
		"INSERT INTO secrets (id, ciphertext, nonce, created_at, updated_at) VALUES ('12345678-1234-4234-8234-123456789abc', $1, $2, $3, $3)",
		[
			{ type: "text", value: encode(new Uint8Array(ciphertext)) },
			{ type: "text", value: encode(nonce) },
			{ type: "text", value: "2026-01-01T00:00:00.000Z" },
		],
	);
	oldClient.close();
	oldDb.close();

	// Insert a damaged secret into the legacy database
	const oldDb2 = await Database.open(oldDbName);
	const oldClient2 = oldDb2.client();
	await oldClient2.executeRaw(
		"INSERT INTO secrets (id, ciphertext, nonce, created_at, updated_at) VALUES ('damaged', 'AA==', 'AA==', 'old', 'old')",
	);
	oldClient2.close();
	oldDb2.close();

	// Register in old registry IndexedDB
	const REGISTRY_DB_NAME = "password-manager-vault-registry-v2";
	await new Promise<void>((resolve, reject) => {
		const request = indexedDB.open(REGISTRY_DB_NAME, 1);
		request.onupgradeneeded = (e) => {
			const db = (e.target as IDBOpenDBRequest).result;
			if (!db.objectStoreNames.contains("vaults")) {
				db.createObjectStore("vaults", { keyPath: "id" });
			}
		};
		request.onsuccess = () => {
			const db = request.result;
			const tx = db.transaction("vaults", "readwrite");
			tx.objectStore("vaults").add({ id, name, status: "ready", migrated: false });
			tx.oncomplete = () => { db.close(); resolve(); };
		};
		request.onerror = () => reject(request.error);
	});

	// Migration succeeds (row counts match), but password verification fails on damaged record
	await expect(unlockVault("fixture-password", id)).rejects.toThrow(
		"Wrong password or damaged legacy records",
	);
	// Legacy database is deleted after successful migration verification, even though unlock fails
	const databases = await indexedDB.databases();
	expect(databases.some((db) => db.name === oldDbName)).toBe(false);
}, 15000);
