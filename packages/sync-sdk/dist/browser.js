// Browser implementation of @ofsdk/sync-sdk
import { Database } from "@ofsdk/sql";
import { StorageClient } from "@ofsdk/storage-client";
import { SyncClient } from "@ofsdk/storage-sync-client";
import { BehaviorSubject, Subject } from "rxjs";
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
        const idp = new URL(config.apiUrl);
        idp.pathname = idp.pathname.replace(/\/storage\/?$/, "/idp");
        this.storageClient = new StorageClient({
            idpBaseUrl: idp,
            storageBaseUrl: config.apiUrl,
            clientId: config.appId,
            bearerToken: config.getToken,
            fetch: (input, init) => fetch(input, { ...init, credentials: "omit", redirect: "error" }),
        });
        // Provide Storage capability token exchange using the StorageClient
        const storageClient = this.storageClient;
        config.getStorageToken = async (resource, actions) => storageClient.getStorageToken(resource, actions);
    }
    /** SQL database sync */
    sql = {
        open: async (databaseId) => {
            const existing = this.databases.get(databaseId);
            if (existing && !existing.isClosed)
                return existing;
            const handle = await this.storageClient.openSqlDatabase(databaseId);
            return this.openOwnedSqlHandle(handle, databaseId);
        },
        create: async (name) => {
            return this.openOwnedSqlHandle(await this.storageClient.createSqlDatabase(name));
        },
        list: async () => {
            const resources = await this.storageClient.listSqlDatabases();
            return resources.map((r) => ({ id: r.id, name: r.name ?? null }));
        },
    };
    async openOwnedSqlHandle(handle, cacheKey = handle.id) {
        const database = await Database.open(handle.id);
        const { apiUrl, getStorageToken } = this.config;
        const dbHandle = new BrowserDatabaseHandle(database, { ...handle.identity, creationEpoch: 0n, schemaFingerprint: new Uint8Array(32) }, {
            storageApiUrl: apiUrl,
            tokenProvider: () => getStorageToken("storage:sync", ["read", "write"]),
        }, true);
        this.databases.set(cacheKey, dbHandle);
        return dbHandle;
    }
    /** KV store sync */
    kv = {
        open: async (storeId) => {
            const existing = this.kvStores.get(storeId);
            if (existing)
                return existing;
            await this.storageClient.openKvStore(storeId);
            const kvHandle = new BrowserKvStoreHandle();
            this.kvStores.set(storeId, kvHandle);
            return kvHandle;
        },
        create: async (name) => {
            const handle = await this.storageClient.createKvStore(name);
            const kvHandle = new BrowserKvStoreHandle();
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
            const existing = this.filesystems.get(fsId);
            if (existing)
                return existing;
            await this.storageClient.openFilesystem(fsId);
            const fsHandle = new BrowserFilesystemHandle();
            this.filesystems.set(fsId, fsHandle);
            return fsHandle;
        },
        create: async (name) => {
            const handle = await this.storageClient.createFilesystem(name);
            const fsHandle = new BrowserFilesystemHandle();
            this.filesystems.set(handle.id, fsHandle);
            return fsHandle;
        },
        list: async () => {
            const resources = await this.storageClient.listFileSystems();
            return resources.map((r) => ({ id: r.id, name: r.name ?? null }));
        },
    };
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
/**
 * Wrap a caller-owned database for an established replica of the given Storage Resource.
 * This does not open storage, authorize, or start sync. Explicit `sync()` is bidirectional
 * and must not be used for first upload or to join independent histories. `close()`
 * stops this handle's sync without closing the original database.
 */
export function createEstablishedSqlHandle(database, identity, options) {
    return new BrowserDatabaseHandle(database, identity, options, false);
}
/** Browser SQL database handle */
class BrowserDatabaseHandle {
    _changes$ = new Subject();
    _state$ = new BehaviorSubject({
        status: "disconnected",
        version: 0,
        vectorClock: {},
    });
    lifecycle = null;
    closed = false;
    database;
    identity;
    options;
    ownsDatabase;
    constructor(database, identity, options, ownsDatabase) {
        this.database = database;
        this.identity = {
            storeId: identity.storeId,
            replicaId: identity.replicaId,
            creationEpoch: identity.creationEpoch,
            schemaFingerprint: new Uint8Array(identity.schemaFingerprint),
        };
        this.options = { storageApiUrl: options.storageApiUrl, tokenProvider: options.tokenProvider };
        this.ownsDatabase = ownsDatabase;
    }
    get id() {
        return this.identity.storeId;
    }
    get isClosed() {
        return this.closed;
    }
    async execute(sql, params) {
        if (this.closed)
            throw new Error("Database closed");
        return execute(this.database, sql, params);
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
    sync(_options) {
        if (this.closed)
            return Promise.resolve();
        return this.getSyncLifecycle().sync();
    }
    async close() {
        if (this.closed)
            return;
        this.closed = true;
        await this.lifecycle?.close();
        if (this.ownsDatabase)
            this.database.close();
        this._changes$.complete();
        this._state$.complete();
    }
    getSyncLifecycle() {
        if (this.lifecycle)
            return this.lifecycle;
        this.lifecycle = new SqlSyncLifecycle(this.database, new SyncClient(this.options), this.identity, {
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
}
/** Browser KV store handle */
class BrowserKvStoreHandle {
    _changes$ = new Subject();
    _state$ = new BehaviorSubject({
        status: "disconnected",
        version: 0,
        vectorClock: {},
    });
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
//# sourceMappingURL=browser.js.map