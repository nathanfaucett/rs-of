// Browser implementation of @ofsdk/sync-sdk

import { Database } from "@ofsdk/sql";
import { type SqlDatabaseHandle, StorageClient } from "@ofsdk/storage-client";
import { type StoreIdentity, SyncClient, type SyncClientOptions } from "@ofsdk/storage-sync-client";
import { BehaviorSubject, type Observable, Subject } from "rxjs";
import type {
  ChangeEvent,
  DatabaseHandle,
  FileEntry,
  FilesystemHandle,
  KvStoreHandle,
  QueryResult,
  SyncConfig,
  SyncOptions,
  SyncSDK,
  SyncState,
} from "./core";
import { execute } from "./sql";
import { SqlSyncLifecycle } from "./sql-sync-lifecycle";

export { SyncError } from "@ofsdk/storage-sync-client";
export {
  createSqlResourceSync,
  type SqlResourceSync,
  type SqlResourceSyncOptions,
  type SqlResourceSyncStatus,
  startSqlResourceSync,
} from "./sql-sync-lifecycle";

/** Document type that must have an id property for database operations. */
type WithId = { id: string; [key: string]: unknown };

/** Browser sync SDK implementation */
export class BrowserSyncSDK implements SyncSDK {
  private storageClient: StorageClient;
  private config: SyncConfig;
  private databases = new Map<string, BrowserDatabaseHandle<unknown>>();
  private kvStores = new Map<string, BrowserKvStoreHandle<unknown>>();
  private filesystems = new Map<string, BrowserFilesystemHandle>();
  private _globalState$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });
  public readonly state$ = this._globalState$.asObservable();

  constructor(config: SyncConfig) {
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
    config.getStorageToken = async (resource: string, actions: ("read" | "write")[]) =>
      storageClient.getStorageToken(resource, actions);
  }

  /** SQL database sync */
  sql = {
    open: async (databaseId: string): Promise<DatabaseHandle> => {
      const existing = this.databases.get(databaseId);
      if (existing && !existing.isClosed) return existing;

      const handle = await this.storageClient.openSqlDatabase(databaseId);
      return this.openOwnedSqlHandle(handle, databaseId);
    },

    create: async (name: string): Promise<DatabaseHandle> => {
      return this.openOwnedSqlHandle(await this.storageClient.createSqlDatabase(name));
    },

    list: async () => {
      const resources = await this.storageClient.listSqlDatabases();
      return resources.map((r) => ({ id: r.id, name: r.name ?? null }));
    },
  };

  private async openOwnedSqlHandle(
    handle: SqlDatabaseHandle,
    cacheKey = handle.id
  ): Promise<DatabaseHandle> {
    const database = await Database.open(handle.id);
    const { apiUrl, getStorageToken } = this.config;
    const dbHandle = new BrowserDatabaseHandle(
      database,
      { ...handle.identity, creationEpoch: 0n, schemaFingerprint: new Uint8Array(32) },
      {
        storageApiUrl: apiUrl,
        tokenProvider: () => getStorageToken("storage:sync", ["read", "write"]),
      },
      true
    );
    this.databases.set(cacheKey, dbHandle);
    return dbHandle;
  }

  /** KV store sync */
  kv = {
    open: async (storeId: string): Promise<KvStoreHandle> => {
      const existing = this.kvStores.get(storeId);
      if (existing) return existing;

      await this.storageClient.openKvStore(storeId);
      const kvHandle = new BrowserKvStoreHandle();
      this.kvStores.set(storeId, kvHandle);
      return kvHandle;
    },

    create: async (name: string): Promise<KvStoreHandle> => {
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
    open: async (fsId: string): Promise<FilesystemHandle> => {
      const existing = this.filesystems.get(fsId);
      if (existing) return existing;

      await this.storageClient.openFilesystem(fsId);
      const fsHandle = new BrowserFilesystemHandle();
      this.filesystems.set(fsId, fsHandle);
      return fsHandle;
    },

    create: async (name: string): Promise<FilesystemHandle> => {
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

  async close(): Promise<void> {
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
export function createEstablishedSqlHandle(
  database: Database,
  identity: StoreIdentity,
  options: SyncClientOptions
): DatabaseHandle {
  return new BrowserDatabaseHandle(database, identity, options, false);
}

/** Browser SQL database handle */
class BrowserDatabaseHandle<T = unknown> implements DatabaseHandle<T> {
  private _changes$ = new Subject<ChangeEvent<T>>();
  private _state$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });
  private lifecycle: SqlSyncLifecycle | null = null;
  private closed: boolean = false;

  private readonly database: Database;
  private readonly identity: StoreIdentity;
  private readonly options: SyncClientOptions;
  private readonly ownsDatabase: boolean;

  constructor(
    database: Database,
    identity: StoreIdentity,
    options: SyncClientOptions,
    ownsDatabase: boolean
  ) {
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

  get id(): string {
    return this.identity.storeId;
  }

  get isClosed(): boolean {
    return this.closed;
  }

  async execute(sql: string, params?: unknown[]): Promise<QueryResult> {
    if (this.closed) throw new Error("Database closed");
    return execute(this.database, sql, params);
  }

  get changes$(): Observable<ChangeEvent<T>> {
    return this._changes$.asObservable();
  }

  async upsert(doc: T): Promise<void> {
    const withId = doc as WithId;
    const { id, ...rest } = withId;
    const keys = Object.keys(rest);
    const columns = keys.join(", ");
    const values = keys.map((k) => rest[k]);
    const placeholders = values.map(() => "?").join(", ");
    const sql = `INSERT OR REPLACE INTO data (id, ${columns}) VALUES (?, ${placeholders})`;
    await this.execute(sql, [id, ...values]);
    this._changes$.next({ type: "local-upsert", doc, version: Date.now() });
  }

  async delete(id: string): Promise<void> {
    await this.execute("DELETE FROM data WHERE id = ?", [id]);
    this._changes$.next({ type: "local-delete", id, version: Date.now() });
  }

  get state$(): Observable<SyncState> {
    return this._state$.asObservable();
  }

  sync(_options?: SyncOptions): Promise<void> {
    if (this.closed) return Promise.resolve();
    return this.getSyncLifecycle().sync();
  }

  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    await this.lifecycle?.close();
    if (this.ownsDatabase) this.database.close();
    this._changes$.complete();
    this._state$.complete();
  }

  private getSyncLifecycle(): SqlSyncLifecycle {
    if (this.lifecycle) return this.lifecycle;
    this.lifecycle = new SqlSyncLifecycle(
      this.database,
      new SyncClient(this.options),
      this.identity,
      {
        connecting: () => this._state$.next({ ...this._state$.value, status: "connecting" }),
        synced: () => this._state$.next({ ...this._state$.value, status: "synced" }),
        failed: (error) =>
          this._state$.next({
            ...this._state$.value,
            status: "error",
            error,
          }),
      }
    );
    return this.lifecycle;
  }
}

/** Browser KV store handle */
class BrowserKvStoreHandle<T = unknown> implements KvStoreHandle<T> {
  private _changes$ = new Subject<ChangeEvent<T>>();
  private _state$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });

  async get(_key: string): Promise<T | undefined> {
    return undefined;
  }

  async set(_key: string, value: T): Promise<void> {
    this._changes$.next({
      type: "local-upsert",
      doc: value,
      version: Date.now(),
    });
  }

  async delete(key: string): Promise<void> {
    this._changes$.next({ type: "local-delete", id: key, version: Date.now() });
  }

  get changes$(): Observable<ChangeEvent<T>> {
    return this._changes$.asObservable();
  }

  get state$(): Observable<SyncState> {
    return this._state$.asObservable();
  }

  async sync(_options?: SyncOptions): Promise<void> {
    this._state$.next({ ...this._state$.value, status: "connecting" });
    this._state$.next({ ...this._state$.value, status: "synced" });
  }

  async close(): Promise<void> {
    this._changes$.complete();
    this._state$.complete();
  }
}

/** Browser filesystem handle */
class BrowserFilesystemHandle implements FilesystemHandle {
  private _changes$ = new Subject<ChangeEvent<FileEntry>>();
  private _state$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });

  async read(_path: string): Promise<Uint8Array> {
    throw new Error("Filesystem not initialized");
  }

  async write(_path: string, _data: Uint8Array): Promise<void> {
    throw new Error("Filesystem not initialized");
  }

  async delete(_path: string): Promise<void> {
    throw new Error("Filesystem not initialized");
  }

  async list(_path: string): Promise<FileEntry[]> {
    throw new Error("Filesystem not initialized");
  }

  get changes$(): Observable<ChangeEvent<FileEntry>> {
    return this._changes$.asObservable();
  }

  get state$(): Observable<SyncState> {
    return this._state$.asObservable();
  }

  async sync(_options?: SyncOptions): Promise<void> {
    this._state$.next({ ...this._state$.value, status: "connecting" });
    this._state$.next({ ...this._state$.value, status: "synced" });
  }

  async close(): Promise<void> {
    this._changes$.complete();
    this._state$.complete();
  }
}

/** Create sync SDK instance for browser */
export async function createSync(config: SyncConfig): Promise<SyncSDK> {
  const sdk = new BrowserSyncSDK(config);
  return sdk;
}
