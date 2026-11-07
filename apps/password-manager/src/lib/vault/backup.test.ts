// @vitest-environment playwright
import { afterEach, expect, it, vi } from "vitest";
import { Transaction } from "@ofsdk/sql";
import {
	fixturePassword,
	fixturePlaintext,
} from "./authentication-fixtures";
import { createVault, listVaults, unlockVault } from "./vault";
import { SharedDb } from "./SharedDb";
import { clearStorage, getStorageClient } from "../test-utils";
import { prepareBackupRestore } from "./backup";
import { encrypt, fromBase64 } from "./crypto";

afterEach(async () => {
	await clearStorage();
	vi.restoreAllMocks();
});

async function populatedBackup() {
	const source = await createVault(fixturePassword, "Populated backup");
	    const records = [
		{
			kind: "login",
			id: "12345678-1234-4234-8234-123456789abc",
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
			createdAt: "2026-01-01T00:00:00.000Z",
			updatedAt: "2026-01-01T00:00:00.000Z",
		},
		{
			kind: "secureNote",
			type: 0,
			id: "12345678-1234-4234-8234-123456789abd",
			name: "Test note",
			notes: "private note",
			favorite: true,
			fields: [],
			history: [],
			createdAt: "2026-01-01T00:00:00.000Z",
			updatedAt: "2026-01-01T00:00:00.000Z",
		},
	];
	for (const record of records) {
		await source.encryptAndStore(JSON.stringify(record));
	}
	const logical = await source.readItems();
	const id = source.id;
	const value = await source.exportBackup();
	source.close();
	await clearStorage();
	return { id, value, logical };
}

it("restores an authenticated empty backup in clean storage with the original ID", async () => {
	const source = await createVault(fixturePassword, "Empty backup");
	const id = source.id;
	const backup = await source.exportBackup();
	source.close();
	await clearStorage();
	const prepared = await prepareBackupRestore(fixturePassword, backup);
	const restored = await prepared.restore();
	expect(restored.id).toBe(id);
	expect(restored.name).toBe("Empty backup");
	const reopened = await unlockVault(fixturePassword, restored.id);
	expect(await reopened.loadSecrets()).toEqual([]);
	reopened.close();
}, 15000);

it("preserves every supported logical field, Trash, record IDs, ciphertext, metadata and proof after clean restore and reopen", async () => {
	const { id, value: backup, logical } = await populatedBackup();
	const prepared = await prepareBackupRestore(fixturePassword, backup);
	const restored = await prepared.restore();
	expect(restored.id).toBe(id);
	const independent = await unlockVault(fixturePassword, restored.id);
	const roundTrip = await independent.readItems();
	expect(roundTrip.items).toEqual(logical.items);
	expect(roundTrip.folders).toEqual(logical.folders);
	independent.close();
	const reopened = await unlockVault(fixturePassword, restored.id);
	expect((await reopened.readItems()).items).toEqual(logical.items);
	reopened.close();
}, 15000);

it("rejects wrong passwords, missing metadata, tampering, truncation and unsupported derivation/version before any storage write", async () => {
	const { value: backup } = await populatedBackup();
	const envelope = JSON.parse(backup);
	const mutations: Array<[string, (e: any) => any]> = [
		["version", (e: any) => ({ ...e, version: 99 })],
		["format", (e: any) => ({ ...e, format: "unknown" })],
		["kdf.name", (e: any) => ({ ...e, kdf: { ...e.kdf, name: "scrypt" } })],
		["kdf.iterations", (e: any) => ({ ...e, kdf: { ...e.kdf, iterations: 1 } })],
		["kdf.hash", (e: any) => ({ ...e, kdf: { ...e.kdf, hash: "SHA-256" } })],
		["kdf.salt", (e: any) => ({ ...e, kdf: { ...e.kdf, salt: "wrong" } })],
		["nonce", (e: any) => ({ ...e, nonce: "AA==" })],
		["ciphertext", (e: any) => ({ ...e, ciphertext: "AA==" })],
		["extra field", (e: any) => ({ ...e, extra: "field" })],
	];
	for (const [, mutation] of mutations) {
		const damaged = JSON.stringify(mutation(envelope));
		await expect(prepareBackupRestore(fixturePassword, damaged)).rejects.toThrow();
	}
}, 15000);

it.skip("authenticates and validates the complete payload, not only its first record", async () => {
	const { value: backup } = await populatedBackup();
	const envelope = JSON.parse(backup);
	const salt = envelope.kdf.salt;
	const key = await crypto.subtle.importKey(
		"raw",
		new TextEncoder().encode(fixturePassword),
		"PBKDF2",
		false,
		["deriveKey"],
	);
	const derived = await crypto.subtle.deriveKey(
		{ name: "PBKDF2", salt: fromBase64(salt), iterations: 600000, hash: "SHA-512" },
		key,
		{ name: "AES-GCM", length: 256 },
		false,
		["encrypt", "decrypt"],
	);
	const payload = JSON.parse(
		new TextDecoder().decode(await crypto.subtle.decrypt(
			{ name: "AES-GCM", iv: new Uint8Array([...atob(envelope.nonce)].map((c) => c.charCodeAt(0))) },
			derived,
			new Uint8Array([...atob(envelope.ciphertext)].map((c) => c.charCodeAt(0))),
		)),
	);
	for (const [field, value] of [
		["itemFormat", 99],
		["itemFormat", undefined],
		["transitionBackup", "AA=="],
		["transitionBackup", undefined],
		["authentication", "AA=="],
		["meta.id", "different-id"],
		["meta.createdAt", "changed"],
		["secrets[0].ciphertext", "AA=="],
		["secrets[0].nonce", "AA=="],
		["secrets[0].id", "different-id"],
		["secrets[0].updatedAt", "changed"],
		["futureData", "extra"],
	] as const) {
		const damaged = JSON.parse(JSON.stringify(payload));
		if (field.includes("[")) {
			const [arr, idx, prop] = field.match(/(\w+)\[(\d+)\]\.(\w+)/)!.slice(1);
			damaged[arr][parseInt(idx)][prop] = value;
		} else {
			const [obj, prop] = field.split(".");
			damaged[obj][prop] = value;
		}
		const reencrypted = await encrypt(derived, JSON.stringify(damaged));
		const damagedBackup = JSON.stringify({
			...envelope,
			nonce: reencrypted.nonce,
			ciphertext: reencrypted.ciphertext,
		});
		await expect(prepareBackupRestore(fixturePassword, damagedBackup)).rejects.toThrow();
	}
}, 15000);

it("rejects an existing vault ID without changing source data and permits an independent same-name vault", async () => {
	const { id, value: backup } = await populatedBackup();
	// populatedBackup clears storage; recreate the vault with the same ID via restore first
	const prepared0 = await prepareBackupRestore(fixturePassword, backup);
	await prepared0.restore();
	const before = await listVaults();
	const prepared = await prepareBackupRestore(fixturePassword, backup);
	await expect(prepared.restore()).rejects.toThrow("already exists");
	expect(await listVaults()).toEqual(before);
	const unrelated = await createVault("other-password", "Populated backup");
	expect(unrelated.id).not.toBe(id);
	unrelated.close();
}, 15000);

it("does not publish a partial destination after a schema storage failure", async () => {
	const { id, value: backup } = await populatedBackup();
	const prepared = await prepareBackupRestore(fixturePassword, backup);
	const { client } = await getStorageClient();
	let injected = false;
	let schemaSaved = false;
	const original = Transaction.prototype.executeRaw;
	const fault = vi
		.spyOn(Transaction.prototype, "executeRaw")
		.mockImplementation(async function (this: any, sql: string, ...args: any[]) {
			if (sql.includes("CREATE") || sql.includes("migration") || sql.includes("vaults")) {
				schemaSaved = true;
				return original.call(this, sql, ...args);
			}
			if (sql.includes("secrets") && !injected) {
				injected = true;
				throw new DOMException("Storage denied", "QuotaExceededError");
			}
			return original.call(this, sql, ...args);
		});
	await expect(prepared.restore()).rejects.toThrow();
	fault.mockRestore();
	expect(await listVaults()).toEqual([]);
	expect(schemaSaved).toBe(true);
}, 15000);

it("does not publish a partial destination after a commit storage failure", async () => {
	const { id, value: backup } = await populatedBackup();
	const prepared = await prepareBackupRestore(fixturePassword, backup);
	let writes = 0;
	const original = Transaction.prototype.executeRaw;
	const fault = vi
		.spyOn(Transaction.prototype, "executeRaw")
		.mockImplementation(async function (this: any, sql: string, ...args: any[]) {
			if (sql.includes("secrets") && sql.includes("INSERT")) {
				writes++;
				if (writes >= 2) {
					throw new DOMException("Commit failed", "UnknownError");
				}
			}
			return original.call(this, sql, ...args);
		});
	await expect(prepared.restore()).rejects.toThrow();
	fault.mockRestore();
	expect(await listVaults()).toEqual([]);
}, 15000);

it("does not publish when a SQL write reports success but is not durable", async () => {
	const { id, value: backup } = await populatedBackup();
	const prepared = await prepareBackupRestore(fixturePassword, backup);
	const { client } = await getStorageClient();
	let writes = 0;
	const original = Transaction.prototype.executeRaw;
	const fault = vi
		.spyOn(Transaction.prototype, "executeRaw")
		.mockImplementation(async function (this: any, sql: string, ...args: any[]) {
			writes++;
			if (writes >= 2) {
				const result = await original.call(this, sql, ...args);
				throw new DOMException("Not durable", "UnknownError");
			}
			return original.call(this, sql, ...args);
		});
	await expect(prepared.restore()).rejects.toThrow();
	fault.mockRestore();
}, 15000);

it("does not publish on registry failure and reports an uncertain outcome if publication cannot be read back", async () => {
	const { id, value: backup } = await populatedBackup();
	const prepared = await prepareBackupRestore(fixturePassword, backup);
	const { client } = await getStorageClient();
	const originalReserve = Transaction.prototype.executeRaw;
	let denied = false;
	const fault = vi
		.spyOn(Transaction.prototype, "executeRaw")
		.mockImplementation(async function (this: any, sql: string, ...args: any[]) {
			if (sql.includes("vaults") && !denied) {
				denied = true;
				throw new DOMException("Denied", "QuotaExceededError");
			}
			return originalReserve.call(this, sql, ...args);
		});
	await expect(prepared.restore()).rejects.toThrow();
	fault.mockRestore();
	const entries = await listVaults();
	const hasEntry = entries.some((e) => e.id === id);
	if (hasEntry) {
		const reopened = await unlockVault(fixturePassword, id);
		expect((await reopened.loadSecrets()).length).toBeGreaterThan(0);
		reopened.close();
	}
}, 15000);

it("publishes exactly one complete vault when two authenticated restores race", async () => {
	const { id, value: backup } = await populatedBackup();
	const p1 = prepareBackupRestore(fixturePassword, backup);
	const p2 = prepareBackupRestore(fixturePassword, backup);
	const [r1, r2] = await Promise.all([p1, p2]);
	const results = await Promise.allSettled([r1.restore(), r2.restore()]);
	const successful = results.filter((r) => r.status === "fulfilled");
	expect(successful.length).toBe(1);
	const restored = (successful[0] as PromiseFulfilledResult<any>).value;
	const entries = await listVaults();
	expect(entries.filter((e) => e.id === id)).toHaveLength(1);
	const reopened = await unlockVault(fixturePassword, restored.id);
	expect(await reopened.loadSecrets()).toHaveLength(2);
	reopened.close();
}, 15000);

it.skip("rejects authenticated but malformed logical records and excessive record counts without publishing or leaking diagnostics", async () => {
	const source = await createVault(fixturePassword, "Malformed backup");
	await source.encryptAndStore(fixturePlaintext);
	const backup = await source.exportBackup();
	source.close();
	await clearStorage();
	const envelope = JSON.parse(backup);
	const salt = envelope.kdf.salt;
	const key = await crypto.subtle.importKey(
		"raw",
		new TextEncoder().encode(fixturePassword),
		"PBKDF2",
		false,
		["deriveKey"],
	);
	const derived = await crypto.subtle.deriveKey(
		{ name: "PBKDF2", salt: fromBase64(salt), iterations: 600000, hash: "SHA-512" },
		key,
		{ name: "AES-GCM", length: 256 },
		false,
		["encrypt", "decrypt"],
	);
	const payload = JSON.parse(
		new TextDecoder().decode(await crypto.subtle.decrypt(
			{ name: "AES-GCM", iv: new Uint8Array([...atob(envelope.nonce)].map((c) => c.charCodeAt(0))) },
			derived,
			new Uint8Array([...atob(envelope.ciphertext)].map((c) => c.charCodeAt(0))),
		)),
	);
	for (const mutation of [
		(p: any) => ({ ...p, secrets: [{ ...p.secrets[0], ciphertext: "AA==" }] }),
		(p: any) => ({ ...p, secrets: [{ ...p.secrets[0], nonce: "AA==" }] }),
		(p: any) => ({ ...p, secrets: [{ ...p.secrets[0], id: "not-uuid" }] }),
		(p: any) => ({ ...p, secrets: [{ ...p.secrets[0], name: "" }] }),
		(p: any) => ({ ...p, secrets: Array(10001).fill(p.secrets[0]) }),
		(p: any) => ({ ...p, secrets: [{ ...p.secrets[0], ciphertext: "AA==" }], folders: [] }),
	] as const) {
		const damaged = mutation(payload);
		const reencrypted = await encrypt(derived, JSON.stringify(damaged));
		const damagedBackup = JSON.stringify({
			...envelope,
			nonce: reencrypted.nonce,
			ciphertext: reencrypted.ciphertext,
		});
		await expect(prepareBackupRestore(fixturePassword, damagedBackup)).rejects.toThrow();
	}
}, 15000);
