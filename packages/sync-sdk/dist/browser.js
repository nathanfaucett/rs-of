// Browser implementation of @ofsdk/sync-sdk
import { BehaviorSubject, Subject } from "rxjs";
import { StorageClient } from "@ofsdk/storage-client";
import { SyncClient, } from "@ofsdk/storage-sync-client";
import initializeSql, { SqlDatabase, SqlSessionConfig, SqlSyncRole, JsSqlSyncTransport, } from "@ofsdk/sql";
/** Browser sync SDK implementation */
export class BrowserSyncSDK {
    config;
    storageClient;
    syncClient = null;
    databases = new Map();
    kvStores = new Map();
    filesystems = new Map();
    _globalState$ = new BehaviorSubject({
        status: "disconnected",
        version: 0,
        vectorClock: {},
    });
    state$ = this._globalState$.asObservable();
    closed = false;
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
    /** Initialize the sync client */
    async initSyncClient() {
        if (this.syncClient)
            return this.syncClient;
        this.syncClient = new SyncClient({
            storageApiUrl: this.config.apiUrl,
            tokenProvider: () => this.config.getToken(),
        });
        return this.syncClient;
    }
    /** SQL database sync */
    sql = {
        open: async (databaseId) => {
            if (this.databases.has(databaseId)) {
                return this.databases.get(databaseId);
            }
            const handle = await this.storageClient.openSqlDatabase(databaseId);
            // Open local WASM database with the resourceId as name
            const sqlDatabase = await SqlDatabase.open(handle.id);
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
            const sqlDatabase = await SqlDatabase.open(handle.id);
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
        this.closed = true;
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
    handle;
    config;
    _changes$ = new Subject();
    _state$ = new BehaviorSubject({
        status: "disconnected",
        version: 0,
        vectorClock: {},
    });
    sqlDatabase = null;
    syncClient = null;
    closed = false;
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
        const client = this.sqlDatabase.client();
        const result = await client.execute(sql, params
            ? JSON.stringify(params.map((value) => {
                if (value === null)
                    return { type: "null" };
                if (typeof value === "string")
                    return { type: "text", value };
                if (typeof value === "boolean")
                    return { type: "bool", value };
                if (typeof value === "bigint" ||
                    (typeof value === "number" && Number.isSafeInteger(value))) {
                    return { type: "integer", value: String(value) };
                }
                throw new TypeError("SQL parameters must be text, booleans, integers or null");
            }))
            : undefined);
        const results = JSON.parse(result);
        const last = results.at(-1) ?? { columns: [], rows: [] };
        return {
            columns: last.columns,
            rows: last.rows.map((row) => row.map((value) => {
                if (value.type === "null")
                    return null;
                if (value.type === "integer") {
                    const integer = BigInt(value.value);
                    return Number.isSafeInteger(Number(integer))
                        ? Number(integer)
                        : integer;
                }
                return value.value;
            })),
        };
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
    async sync(options) {
        if (!this.sqlDatabase || this.closed)
            return;
        this._state$.next({ ...this._state$.value, status: "connecting" });
        try {
            const syncClient = await this.getSyncClient();
            const syncSession = await syncClient.attach({
                identity: this.handle.storeIdentity,
                direction: "Bidirectional",
            });
            const transport = new JsSqlSyncTransport({
                send: (data) => {
                    syncSession.send(data);
                    return Promise.resolve();
                },
                receive: () => syncSession.receive(),
            });
            const config = new SqlSessionConfig();
            await this.sqlDatabase.synchronize(transport, config, SqlSyncRole.Initiator);
            this._state$.next({ ...this._state$.value, status: "synced" });
        }
        catch (error) {
            this._state$.next({
                ...this._state$.value,
                status: "error",
                error: error,
            });
            throw error;
        }
    }
    async close() {
        this.closed = true;
        this._changes$.complete();
        this._state$.complete();
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
    handle;
    config;
    _changes$ = new Subject();
    _state$ = new BehaviorSubject({
        status: "disconnected",
        version: 0,
        vectorClock: {},
    });
    closed = false;
    constructor(handle, config) {
        this.handle = handle;
        this.config = config;
    }
    async get(key) {
        return undefined;
    }
    async set(key, value) {
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
    async sync(options) {
        this._state$.next({ ...this._state$.value, status: "connecting" });
        this._state$.next({ ...this._state$.value, status: "synced" });
    }
    async close() {
        this.closed = true;
        this._changes$.complete();
        this._state$.complete();
    }
}
/** Browser filesystem handle */
class BrowserFilesystemHandle {
    handle;
    config;
    _changes$ = new Subject();
    _state$ = new BehaviorSubject({
        status: "disconnected",
        version: 0,
        vectorClock: {},
    });
    fsHandle = null;
    closed = false;
    constructor(handle, config) {
        this.handle = handle;
        this.config = config;
    }
    async read(path) {
        if (!this.fsHandle)
            throw new Error("Filesystem not initialized");
        return this.fsHandle.read(path);
    }
    async write(path, data) {
        if (!this.fsHandle)
            throw new Error("Filesystem not initialized");
        await this.fsHandle.write(path, data);
        this._changes$.next({
            type: "local-upsert",
            doc: {
                path,
                isDirectory: false,
                size: data.length,
                modified: Date.now(),
            },
        });
    }
    async delete(path) {
        if (!this.fsHandle)
            throw new Error("Filesystem not initialized");
        await this.fsHandle.delete(path);
        this._changes$.next({ type: "local-delete", id: path });
    }
    async list(path) {
        if (!this.fsHandle)
            throw new Error("Filesystem not initialized");
        return this.fsHandle.list(path);
    }
    get changes$() {
        return this._changes$.asObservable();
    }
    get state$() {
        return this._state$.asObservable();
    }
    async sync(options) {
        this._state$.next({ ...this._state$.value, status: "connecting" });
        this._state$.next({ ...this._state$.value, status: "synced" });
    }
    async close() {
        this.closed = true;
        if (this.fsHandle) {
            this.fsHandle.close();
        }
        this._changes$.complete();
        this._state$.complete();
    }
}
/** Create sync SDK instance for browser */
export async function createSync(config) {
    await initializeSql();
    const sdk = new BrowserSyncSDK(config);
    return sdk;
}
