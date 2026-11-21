// Browser implementation of @ofsdk/sync-sdk
import { BehaviorSubject, Subject } from "rxjs";
import { StorageClient } from "@ofsdk/storage-client";
import { SyncClient } from "@ofsdk/storage-sync-client";
import { Database } from "@ofsdk/sql";
import { execute } from "./sql";
import { SqlSyncLifecycle } from "./sql-sync-lifecycle";
/** Browser sync SDK implementation */
export class BrowserSyncSDK {
    storageClient;
    config;
    databases = new Map();
    kvStores = new Map();
    filesystems = new Map();
    _globalState$ = new BehaviorSubject({
        status: "disconnected",
        version: 0,
        vectorClock: {},
    });
    state$ = this._globalState$.asObservable();
    constructor(config) {
        this.config = config;
        this.storageClient = new StorageClient({
            idpBaseUrl: config.apiUrl.replace("/storage", "/idp"),
            storageBaseUrl: config.apiUrl,
            clientId: config.appId,
            bearerToken: config.getToken,
        });
        // Provide Storage capability token exchange using the StorageClient
        const storageClient = this.storageClient;
        config.getStorageToken = async (resource, actions) => storageClient.getStorageToken(resource, actions);
    }
    /** SQL database sync */
    sql = {
        open: async (databaseId) => {
            if (this.databases.has(databaseId)) {
                return this.databases.get(databaseId);
            }
            const handle = await this.storageClient.openSqlDatabase(databaseId);
            // Open local WASM database with the resourceId as name
            const sqlDatabase = await Database.open(handle.id);
            const storeIdentity = {
                storeId: handle.identity.storeId,
                replicaId: handle.identity.replicaId,
                creationEpoch: 0n,
                schemaFingerprint: new Uint8Array(32),
            };
            const dbHandle = new BrowserDatabaseHandle({
                sqlDatabase,
                resourceId: handle.id,
                storeIdentity,
            }, this.config);
            this.databases.set(databaseId, dbHandle);
            return dbHandle;
        },
        create: async (name) => {
            const handle = await this.storageClient.createSqlDatabase(name);
            const sqlDatabase = await Database.open(handle.id);
            const storeIdentity = {
                storeId: handle.identity.storeId,
                replicaId: handle.identity.replicaId,
                creationEpoch: 0n,
                schemaFingerprint: new Uint8Array(32),
            };
            const dbHandle = new BrowserDatabaseHandle({
                sqlDatabase,
                resourceId: handle.id,
                storeIdentity,
            }, this.config);
            this.databases.set(handle.id, dbHandle);
            return dbHandle;
        },
        list: async () => {
            const resources = await this.storageClient.listSqlDatabases();
            return resources.map((r) => ({ id: r.id, name: r.name ?? null }));
        },
    };
    /** KV store sync */
    kv = {
        open: async (storeId) => {
            if (this.kvStores.has(storeId)) {
                return this.kvStores.get(storeId);
            }
            const handle = await this.storageClient.openKvStore(storeId);
            const storeIdentity = {
                storeId: handle.id,
                replicaId: crypto.randomUUID(),
                creationEpoch: 0n,
                schemaFingerprint: new Uint8Array(32),
            };
            const kvHandle = new BrowserKvStoreHandle({ resourceId: handle.id, storeIdentity }, this.config);
            this.kvStores.set(storeId, kvHandle);
            return kvHandle;
        },
        create: async (name) => {
            const handle = await this.storageClient.createKvStore(name);
            const storeIdentity = {
                storeId: handle.id,
                replicaId: crypto.randomUUID(),
                creationEpoch: 0n,
                schemaFingerprint: new Uint8Array(32),
            };
            const kvHandle = new BrowserKvStoreHandle({ resourceId: handle.id, storeIdentity }, this.config);
            this.kvStores.set(handle.id, kvHandle);
            return kvHandle;
        },
        list: async () => {
            return this.storageClient.listKvStores();
        },
    };
    /** Filesystem sync */
    fs = {
        open: async (fsId) => {
            if (this.filesystems.has(fsId)) {
                return this.filesystems.get(fsId);
            }
            const handle = await this.storageClient.openFilesystem(fsId);
            const storeIdentity = {
                storeId: handle.id,
                replicaId: crypto.randomUUID(),
                creationEpoch: 0n,
                schemaFingerprint: new Uint8Array(32),
            };
            const fsHandle = new BrowserFilesystemHandle({ resourceId: handle.id, storeIdentity }, this.config);
            this.filesystems.set(fsId, fsHandle);
            return fsHandle;
        },
        create: async (name) => {
            const handle = await this.storageClient.createFilesystem(name);
            const storeIdentity = {
                storeId: handle.id,
                replicaId: crypto.randomUUID(),
                creationEpoch: 0n,
                schemaFingerprint: new Uint8Array(32),
            };
            const fsHandle = new BrowserFilesystemHandle({ resourceId: handle.id, storeIdentity }, this.config);
            this.filesystems.set(handle.id, fsHandle);
            return fsHandle;
        },
        list: async () => {
            const resources = await this.storageClient.listFileSystems();
            return resources.map((r) => ({ id: r.id, name: r.name ?? null }));
        },
    };
    async syncToDevice(resourceId, targetDeviceId) {
        const session = await this.storageClient.syncToDevice(resourceId, targetDeviceId);
        return {
            sessionId: session.sessionId,
            resourceId: session.resourceId,
            targetDevice: session.targetDevice,
            status: session.status.toLowerCase(),
        };
    }
    async close() {
        for (const db of this.databases.values()) {
            await db.close();
        }
        for (const kv of this.kvStores.values()) {
            await kv.close();
        }
        for (const fs of this.filesystems.values()) {
            await fs.close();
        }
        this.databases.clear();
        this.kvStores.clear();
        this.filesystems.clear();
        this._globalState$.complete();
    }
}
/** Browser SQL database handle */
class BrowserDatabaseHandle {
    _changes$ = new Subject();
    _state$ = new BehaviorSubject({
        status: "disconnected",
        version: 0,
        vectorClock: {},
    });
    sqlDatabase = null;
    syncClient = null;
    lifecycle = null;
    closed = false;
    handle;
    config;
    constructor(handle, config) {
        this.handle = handle;
        this.config = config;
        this.sqlDatabase = handle.sqlDatabase;
    }
    get id() {
        return this.handle.resourceId;
    }
    async execute(sql, params) {
        if (!this.sqlDatabase)
            throw new Error("Database closed");
        return execute(this.sqlDatabase, sql, params);
    }
    get changes$() {
        return this._changes$.asObservable();
    }
    async upsert(doc) {
        const withId = doc;
        const { id, ...rest } = withId;
        const keys = Object.keys(rest);
        const columns = keys.join(", ");
        const values = keys.map((k) => rest[k]);
        const placeholders = values.map(() => "?").join(", ");
        const sql = `INSERT OR REPLACE INTO data (id, ${columns}) VALUES (?, ${placeholders})`;
        await this.execute(sql, [id, ...values]);
        this._changes$.next({ type: "local-upsert", doc, version: Date.now() });
    }
    async delete(id) {
        await this.execute("DELETE FROM data WHERE id = ?", [id]);
        this._changes$.next({ type: "local-delete", id, version: Date.now() });
    }
    get state$() {
        return this._state$.asObservable();
    }
    async sync(_options) {
        if (!this.sqlDatabase || this.closed)
            return;
        const lifecycle = await this.getSyncLifecycle();
        await lifecycle.sync();
    }
    async close() {
        this.closed = true;
        await this.lifecycle?.close();
        this._changes$.complete();
        this._state$.complete();
    }
    async getSyncLifecycle() {
        if (this.lifecycle)
            return this.lifecycle;
        const client = await this.getSyncClient();
        this.lifecycle = new SqlSyncLifecycle(this.sqlDatabase, client, this.handle.storeIdentity, {
            connecting: () => this._state$.next({ ...this._state$.value, status: "connecting" }),
            synced: () => this._state$.next({ ...this._state$.value, status: "synced" }),
            failed: (error) => this._state$.next({
                ...this._state$.value,
                status: "error",
                error,
            }),
        });
        return this.lifecycle;
    }
    async getSyncClient() {
        if (this.syncClient)
            return this.syncClient;
        this.syncClient = new SyncClient({
            storageApiUrl: this.config.apiUrl,
            tokenProvider: () => this.config.getStorageToken("storage:sync", ["read", "write"]),
        });
        return this.syncClient;
    }
}
/** Browser KV store handle */
class BrowserKvStoreHandle {
    _changes$ = new Subject();
    _state$ = new BehaviorSubject({
        status: "disconnected",
        version: 0,
        vectorClock: {},
    });
    constructor(_handle, _config) { }
    async get(_key) {
        return undefined;
    }
    async set(_key, value) {
        this._changes$.next({
            type: "local-upsert",
            doc: value,
            version: Date.now(),
        });
    }
    async delete(key) {
        this._changes$.next({ type: "local-delete", id: key, version: Date.now() });
    }
    get changes$() {
        return this._changes$.asObservable();
    }
    get state$() {
        return this._state$.asObservable();
    }
    async sync(_options) {
        this._state$.next({ ...this._state$.value, status: "connecting" });
        this._state$.next({ ...this._state$.value, status: "synced" });
    }
    async close() {
        this._changes$.complete();
        this._state$.complete();
    }
}
/** Browser filesystem handle */
class BrowserFilesystemHandle {
    _changes$ = new Subject();
    _state$ = new BehaviorSubject({
        status: "disconnected",
        version: 0,
        vectorClock: {},
    });
    constructor(_handle, _config) { }
    async read(_path) {
        throw new Error("Filesystem not initialized");
    }
    async write(_path, _data) {
        throw new Error("Filesystem not initialized");
    }
    async delete(_path) {
        throw new Error("Filesystem not initialized");
    }
    async list(_path) {
        throw new Error("Filesystem not initialized");
    }
    get changes$() {
        return this._changes$.asObservable();
    }
    get state$() {
        return this._state$.asObservable();
    }
    async sync(_options) {
        this._state$.next({ ...this._state$.value, status: "connecting" });
        this._state$.next({ ...this._state$.value, status: "synced" });
    }
    async close() {
        this._changes$.complete();
        this._state$.complete();
    }
}
/** Create sync SDK instance for browser */
export async function createSync(config) {
    const sdk = new BrowserSyncSDK(config);
    return sdk;
}
