import { getSharedClient } from "$lib/vault/storage/migrate-to-shared";

const SHARED_DB_NAME = "password-manager-v1";

export async function clearStorage(): Promise<void> {
	// Delete the shared DB to free up quota
	await new Promise<void>((resolve, reject) => {
		const request = indexedDB.deleteDatabase(SHARED_DB_NAME);
		request.onsuccess = () => resolve();
		request.onerror = () => reject(request.error);
		request.onblocked = () => reject(new Error("Shared database still open"));
	});

	// Delete any legacy per-vault databases
	for (const { name } of await indexedDB.databases()) {
		if (!name) continue;
		if (name === SHARED_DB_NAME) continue;
		await new Promise<void>((resolve, reject) => {
			const request = indexedDB.deleteDatabase(name);
			request.onsuccess = () => resolve();
			request.onerror = () => reject(request.error);
			request.onblocked = () => reject(new Error("Test database still open"));
		});
	}
}

export async function getStorageClient(): Promise<{ client: import("@ofsdk/sql").Client; close: () => void }> {
	const client = await getSharedClient();
	return {
		client,
		close() {
			client.close();
		},
	};
}
