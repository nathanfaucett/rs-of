import { listVaults } from "$lib/vault/vault";
import { SharedDb } from "$lib/vault/SharedDb";

const SHARED_DB_NAME = "password-manager-v1";
const REGISTRY_NAME = "password-manager-vault-registry-v1";

export async function clearStorage(): Promise<void> {
	// First, close and reset SharedDb singleton
	const sharedDb = SharedDb.getInstance();
	sharedDb.close();
	SharedDb.resetForTesting();

	// Delete the shared DB to free up quota
	await new Promise<void>((resolve, reject) => {
		const request = indexedDB.deleteDatabase(SHARED_DB_NAME);
		request.onsuccess = () => resolve();
		request.onerror = () => reject(request.error);
		request.onblocked = () => reject(new Error("Shared database still open"));
	});

	// Clear the registry
	await new Promise<void>((resolve, reject) => {
		const request = indexedDB.deleteDatabase(REGISTRY_NAME);
		request.onsuccess = () => resolve();
		request.onerror = () => reject(request.error);
		request.onblocked = () => reject(new Error("Registry database still open"));
	});

	// Delete any legacy per-vault databases
	for (const { name } of await indexedDB.databases()) {
		if (!name) continue;
		if (name === SHARED_DB_NAME) continue;
		if (name === REGISTRY_NAME) continue;
		await new Promise<void>((resolve, reject) => {
			const request = indexedDB.deleteDatabase(name);
			request.onsuccess = () => resolve();
			request.onerror = () => reject(request.error);
			request.onblocked = () => reject(new Error("Test database still open"));
		});
	}
}

export async function getStorageClient(id: string) {
	const sharedDb = SharedDb.getInstance();
	const client = await sharedDb.getClient(id);
	return {
		client,
		// Don't close the client - it's managed by SharedDb
		close() {
			// No-op: clients are managed by SharedDb
		},
	};
}

export function prefixed(id: string, table: string): string {
	return `vault_${id.replace(/-/g, "")}_${table}`;
}
