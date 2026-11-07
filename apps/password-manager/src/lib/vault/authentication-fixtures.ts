import { Database } from "@ofsdk/sql";
import { vaultMigrations } from "./migrations";
import { add } from "./registry";

export const fixturePassword = "synthetic-master-password";
export const fixturePlaintext = JSON.stringify({
	id: "12345678-1234-4234-8234-123456789abc",
	name: "Synthetic legacy login",
	secret: "synthetic legacy encrypted content",
	createdAt: "2026-01-01T00:00:00.000Z",
	updatedAt: "2026-01-01T00:00:00.000Z",
});

function base64(bytes: Uint8Array): string {
	return btoa(String.fromCharCode(...bytes));
}

/**
 * Creates a legacy per-vault database (password-manager-local-${id}) with v5 schema.
 * This simulates a vault created before the single-DB consolidation.
 * The new unlockVault() will detect this and migrate it to the shared DB.
 */
export async function legacyVault(
	name: string,
	populated: boolean,
): Promise<string> {
	const id = crypto.randomUUID();
	const salt = crypto.getRandomValues(new Uint8Array(16));
	const material = await crypto.subtle.importKey(
		"raw",
		new TextEncoder().encode(fixturePassword),
		"PBKDF2",
		false,
		["deriveKey"],
	);
	const key = await crypto.subtle.deriveKey(
		{ name: "PBKDF2", salt, iterations: 600_000, hash: "SHA-512" },
		material,
		{ name: "AES-GCM", length: 256 },
		false,
		["encrypt"],
	);
	// Create legacy per-vault database with old naming scheme
	const oldDbName = `password-manager-local-${id}`;
	const db = await Database.open(oldDbName);
	const client = db.client();
	try {
		await client.migrate(vaultMigrations);
		await client.execute(
			"INSERT INTO vault_meta (id, vault_id, salt, iv, created_at) VALUES (1, $1, $2, $3, $4)",
			[
				{ type: "text", value: id },
				{ type: "text", value: base64(salt) },
				{ type: "text", value: base64(new Uint8Array(12)) },
				{ type: "text", value: "2026-01-01T00:00:00.000Z" },
			],
		);
		if (populated) {
			const nonce = crypto.getRandomValues(new Uint8Array(12));
			const ciphertext = await crypto.subtle.encrypt(
				{ name: "AES-GCM", iv: nonce },
				key,
				new TextEncoder().encode(fixturePlaintext),
			);
			await client.execute(
				"INSERT INTO secrets (id, ciphertext, nonce, created_at, updated_at) VALUES ('12345678-1234-4234-8234-123456789abc', $1, $2, $3, $3)",
				[
					{ type: "text", value: base64(new Uint8Array(ciphertext)) },
					{ type: "text", value: base64(nonce) },
					{ type: "text", value: "2026-01-01T00:00:00.000Z" },
				],
			);
		}
	} finally {
		client.close();
		db.close();
	}
	// Register the vault in the registry so it appears in the vault list
	await add({
		id,
		name,
		status: "ready",
		migrated: false,
	});
	return id;
}
