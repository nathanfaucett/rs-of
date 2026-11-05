import { Database } from "@ofsdk/sql";

const REGISTRY = "password-manager-vault-registry-v1";
const STORE = "vaults";
const LEGACY_PREFIX = "ofdb:sql:password-manager-vault-";

export interface VaultIdentity {
    id: string;
    name: string;
    databaseName: string;
    previousDatabaseName?: string;
    status: "pending" | "ready";
}

function storageError(cause: unknown): Error {
    return new Error(
        "Cannot access the local vault registry. Allow browser storage and check free space, close other app tabs, then reload. Do not clear browser data; existing vaults are not deleted.",
        { cause },
    );
}

async function registry(): Promise<IDBDatabase> {
    return new Promise<IDBDatabase>((resolve, reject) => {
        let blocked = false;
        const request = indexedDB.open(REGISTRY, 1);
        request.onupgradeneeded = () => {
            const store = request.result.createObjectStore(STORE, {
                keyPath: "id",
            });
            store.createIndex("databaseName", "databaseName", { unique: true });
        };
        request.onsuccess = () => {
            if (blocked) request.result.close();
            else resolve(request.result);
        };
        request.onerror = () => reject(storageError(request.error));
        request.onblocked = () => {
            blocked = true;
            reject(
                new Error(
                    "Registry upgrade blocked. Close other app tabs and reload. Do not clear browser data.",
                ),
            );
        };
    }).catch((cause) => {
        throw storageError(cause);
    });
}

async function entries(): Promise<VaultIdentity[]> {
    const db = await registry();
    try {
        return await new Promise((resolve, reject) => {
            const tx = db.transaction(STORE, "readonly");
            const request = tx.objectStore(STORE).getAll();
            tx.oncomplete = () => {
                const values: VaultIdentity[] = request.result;
                if (
                    values.some(
                        (v) =>
                            !v ||
                            typeof v.id !== "string" ||
                            !v.id ||
                            typeof v.name !== "string" ||
                            !v.name ||
                            typeof v.databaseName !== "string" ||
                            !v.databaseName ||
                            (v.previousDatabaseName !== undefined &&
                                (typeof v.previousDatabaseName !== "string" ||
                                    !v.previousDatabaseName)) ||
                            !["pending", "ready"].includes(v.status),
                    )
                ) {
                    reject(storageError("Invalid registry entry"));
                } else resolve(values);
            };
            tx.onabort = () => reject(storageError(tx.error));
            tx.onerror = () => reject(storageError(tx.error));
        });
    } finally {
        db.close();
    }
}

async function add(entry: VaultIdentity): Promise<void> {
    const db = await registry();
    try {
        await new Promise<void>((resolve, reject) => {
            const tx = db.transaction(STORE, "readwrite");
            tx.objectStore(STORE).add(entry);
            tx.oncomplete = () => resolve();
            tx.onabort = () => reject(storageError(tx.error));
            tx.onerror = () => reject(storageError(tx.error));
        });
    } catch (cause) {
        throw storageError(cause);
    } finally {
        db.close();
    }
}

export async function publishRestoredVault(
    entry: VaultIdentity,
): Promise<void> {
    try {
        await add(entry);
    } catch {
        throw new Error(
            "Restore publication was not acknowledged. Reload the selector and check the vault ID before retrying. Existing vaults were not replaced. Keep the backup; do not clear browser data.",
        );
    }
    try {
        const saved = await findVault(entry.id);
        if (JSON.stringify(saved) !== JSON.stringify(entry))
            throw new Error("Registry mismatch");
    } catch {
        throw new Error(
            "Restore outcome is uncertain. Reload the selector and verify the vault ID before retrying. Keep the backup; do not clear browser data.",
        );
    }
}

export async function publishItemTransition(
    source: VaultIdentity,
    databaseName: string,
): Promise<void> {
    const entry: VaultIdentity = {
        ...source,
        databaseName,
        previousDatabaseName: source.previousDatabaseName ?? source.databaseName,
    };
    const db = await registry();
    try {
        await new Promise<void>((resolve, reject) => {
            const tx = db.transaction(STORE, "readwrite");
            const store = tx.objectStore(STORE);
            const request = store.get(source.id);
            request.onsuccess = () => {
                if (JSON.stringify(request.result) !== JSON.stringify(source)) {
                    tx.abort();
                    return;
                }
                try {
                    store.put(entry);
                } catch {
                    tx.abort();
                }
            };
            tx.oncomplete = () => resolve();
            tx.onabort = () => reject(storageError(tx.error));
            tx.onerror = () => reject(storageError(tx.error));
        });
        if (JSON.stringify(await findVault(entry.id)) !== JSON.stringify(entry))
            throw storageError("Transition registry mismatch");
    } finally {
        db.close();
    }
}

export async function reserveVault(name: string): Promise<VaultIdentity> {
    const displayName = name.trim();
    if (!displayName) throw new Error("Vault name is required.");
    const id = crypto.randomUUID();
    const entry: VaultIdentity = {
        id,
        name: displayName,
        databaseName: `password-manager-local-${id}`,
        status: "pending",
    };
    await add(entry);
    return entry;
}

export async function completeRegistration(
    entry: VaultIdentity,
): Promise<void> {
    if (entry.status === "ready") return;
    const db = await registry();
    try {
        await new Promise<void>((resolve, reject) => {
            const tx = db.transaction(STORE, "readwrite");
            const store = tx.objectStore(STORE);
            const request = store.get(entry.id);
            request.onsuccess = () => {
                const saved: VaultIdentity | undefined = request.result;
                if (
                    !saved ||
                    saved.databaseName !== entry.databaseName ||
                    saved.name !== entry.name
                ) {
                    tx.abort();
                    return;
                }
                try {
                    store.put({ ...saved, status: "ready" });
                } catch {
                    tx.abort();
                }
            };
            tx.oncomplete = () => resolve();
            tx.onabort = () => reject(storageError(tx.error));
            tx.onerror = () => reject(storageError(tx.error));
        });
    } finally {
        db.close();
    }
}

export async function findVault(id: string): Promise<VaultIdentity> {
    const entry = (await entries()).find((v) => v.id === id);
    if (!entry)
        throw new Error(
            "Vault identity not found. Reload the vault selector; do not create a replacement over existing storage.",
        );
    return entry;
}

export async function listVaultIdentities(): Promise<VaultIdentity[]> {
    let saved = await entries();
    let databases: IDBDatabaseInfo[];
    try {
        databases = await indexedDB.databases();
    } catch (cause) {
        throw storageError(cause);
    }
    for (const database of databases) {
        if (!database.name?.startsWith(LEGACY_PREFIX)) continue;
        const databaseName = database.name.slice("ofdb:sql:".length);
        if (
            saved.some(
                (v) =>
                    v.databaseName === databaseName ||
                    v.previousDatabaseName === databaseName,
            )
        )
            continue;
        const db = await Database.open(databaseName);
        const client = db.client();
        try {
            const tables = await client.execute(
                "SELECT name FROM tables WHERE name = 'vault_meta'",
            );
            if (!tables[0]?.rows.length) continue;
            const result = await client.execute(
                "SELECT vault_id FROM vault_meta WHERE id = 1",
            );
            if (!result[0]?.rows.length) continue;
            const id = result[0].rows[0][0];
            if (id?.type !== "text" || !id.value)
                throw storageError("Invalid existing vault identity");
            const entry: VaultIdentity = {
                id: id.value,
                name: database.name.slice(LEGACY_PREFIX.length),
                databaseName,
                status: "ready",
            };
            try {
                await add(entry);
            } catch (cause) {
                const concurrent = (await entries()).find(
                    (v) => v.id === entry.id,
                );
                if (
                    concurrent &&
                    concurrent.databaseName !== entry.databaseName
                ) {
                    throw new Error(
                        "The same vault ID exists in different databases. Recover from your backup or inspect the source databases; neither source was changed.",
                        { cause },
                    );
                }
                if (!concurrent) throw cause;
            }
            saved = await entries();
        } catch (cause) {
            throw new Error(
                "Cannot register an existing local vault. Retry after checking browser storage and app version. The source vault is unchanged; do not clear browser data.",
                { cause },
            );
        } finally {
            client.close();
            db.close();
        }
    }
    return saved;
}
