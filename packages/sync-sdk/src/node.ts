// Node.js implementation of @ofsdk/sync-sdk

import { EventEmitter } from "node:events";
import { type Observable, Subject, BehaviorSubject } from "rxjs";
import { StorageClient } from "@ofsdk/storage-client";
import { SyncClient, type StoreIdentity } from "@ofsdk/storage-sync-client";
import {
  Database,
  JsPushSyncTransport,
  PushSynchronizeConfig,
} from "@ofsdk/sql";
import type {
  SyncConfig,
  SyncSDK,
  DatabaseHandle,
  KvStoreHandle,
  FilesystemHandle,
  ChangeEvent,
  SyncState,
  SyncOptions,
  SyncSession,
  QueryResult,
  FileEntry,
} from "./core";
import { execute } from "./sql";

/** Document type that must have an id property for database operations. */
type WithId = { id: string; [key: string]: unknown };

/** Node.js sync SDK implementation */
export class NodeSyncSDK extends EventEmitter implements SyncSDK {
  private storageClient: StorageClient;
  private config: SyncConfig;
  private databases = new Map<string, NodeDatabaseHandle<unknown>>();
  private kvStores = new Map<string, NodeKvStoreHandle<unknown>>();
  private filesystems = new Map<string, NodeFilesystemHandle>();
  private _globalState$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });
  public readonly state$ = this._globalState$.asObservable();

  constructor(config: SyncConfig) {
    super();
    this.config = config;
    this.storageClient = new StorageClient({
      idpBaseUrl: config.apiUrl.replace("/storage", "/idp"),
      storageBaseUrl: config.apiUrl,
      clientId: config.appId,
      bearerToken: config.getToken,
    });
  }

  /** SQL database sync */
  sql = {
    open: async (databaseId: string): Promise<DatabaseHandle> => {
      if (this.databases.has(databaseId)) {
        return this.databases.get(databaseId)!;
      }

      const handle = await this.storageClient.openSqlDatabase(databaseId);
      const sqlDatabase = await Database.open(handle.id);
      const storeIdentity: StoreIdentity = {
        storeId: handle.identity.storeId,
        replicaId: handle.identity.replicaId,
        creationEpoch: 0n,
        schemaFingerprint: new Uint8Array(32),
      };
      const dbHandle = new NodeDatabaseHandle(
        { sqlDatabase, resourceId: handle.id, storeIdentity },
        this.config,
      );
      this.databases.set(databaseId, dbHandle);
      return dbHandle;
    },

    create: async (name: string): Promise<DatabaseHandle> => {
      const handle = await this.storageClient.createSqlDatabase(name);
      const sqlDatabase = await Database.open(handle.id);
      const storeIdentity: StoreIdentity = {
        storeId: handle.identity.storeId,
        replicaId: handle.identity.replicaId,
        creationEpoch: 0n,
        schemaFingerprint: new Uint8Array(32),
      };
      const dbHandle = new NodeDatabaseHandle(
        { sqlDatabase, resourceId: handle.id, storeIdentity },
        this.config,
      );
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
    open: async (storeId: string): Promise<KvStoreHandle> => {
      if (this.kvStores.has(storeId)) {
        return this.kvStores.get(storeId)!;
      }

      const handle = await this.storageClient.openKvStore(storeId);
      const storeIdentity: StoreIdentity = {
        storeId: handle.id,
        replicaId: crypto.randomUUID(),
        creationEpoch: 0n,
        schemaFingerprint: new Uint8Array(32),
      };
      const kvHandle = new NodeKvStoreHandle(
        { resourceId: handle.id, storeIdentity },
        this.config,
      );
      this.kvStores.set(storeId, kvHandle);
      return kvHandle;
    },

    create: async (name: string): Promise<KvStoreHandle> => {
      const handle = await this.storageClient.createKvStore(name);
      const storeIdentity: StoreIdentity = {
        storeId: handle.id,
        replicaId: crypto.randomUUID(),
        creationEpoch: 0n,
        schemaFingerprint: new Uint8Array(32),
      };
      const kvHandle = new NodeKvStoreHandle(
        { resourceId: handle.id, storeIdentity },
        this.config,
      );
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
      if (this.filesystems.has(fsId)) {
        return this.filesystems.get(fsId)!;
      }

      const handle = await this.storageClient.openFilesystem(fsId);
      const storeIdentity: StoreIdentity = {
        storeId: handle.id,
        replicaId: crypto.randomUUID(),
        creationEpoch: 0n,
        schemaFingerprint: new Uint8Array(32),
      };
      const fsHandle = new NodeFilesystemHandle(
        { resourceId: handle.id, storeIdentity },
        this.config,
      );
      this.filesystems.set(fsId, fsHandle);
      return fsHandle;
    },

    create: async (name: string): Promise<FilesystemHandle> => {
      const handle = await this.storageClient.createFilesystem(name);
      const storeIdentity: StoreIdentity = {
        storeId: handle.id,
        replicaId: crypto.randomUUID(),
        creationEpoch: 0n,
        schemaFingerprint: new Uint8Array(32),
      };
      const fsHandle = new NodeFilesystemHandle(
        { resourceId: handle.id, storeIdentity },
        this.config,
      );
      this.filesystems.set(handle.id, fsHandle);
      return fsHandle;
    },

    list: async () => {
      const resources = await this.storageClient.listFileSystems();
      return resources.map((r) => ({ id: r.id, name: r.name ?? null }));
    },
  };

  async syncToDevice(
    resourceId: string,
    targetDeviceId: string,
  ): Promise<SyncSession> {
    const session = await this.storageClient.syncToDevice(
      resourceId,
      targetDeviceId,
    );
    return {
      sessionId: session.sessionId,
      resourceId: session.resourceId,
      targetDevice: session.targetDevice,
      status: session.status.toLowerCase() as SyncSession["status"],
    };
  }

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

/** Node.js SQL database handle */
class NodeDatabaseHandle<T = unknown> implements DatabaseHandle<T> {
  private _changes$ = new Subject<ChangeEvent<T>>();
  private _state$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });
  private sqlDatabase: Database | null = null;
  private syncClient: SyncClient | null = null;
  private closed = false;
  private handle: {
    sqlDatabase: Database;
    resourceId: string;
    storeIdentity: StoreIdentity;
  };
  private config: SyncConfig;

  constructor(
    handle: {
      sqlDatabase: Database;
      resourceId: string;
      storeIdentity: StoreIdentity;
    },
    config: SyncConfig,
  ) {
    this.handle = handle;
    this.config = config;
    this.sqlDatabase = handle.sqlDatabase;
  }

  get id(): string {
    return this.handle.resourceId;
  }

  async execute(sql: string, params?: unknown[]): Promise<QueryResult> {
    if (!this.sqlDatabase) throw new Error("Database closed");
    return execute(this.sqlDatabase, sql, params);
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
    const sqlQuery = `INSERT OR REPLACE INTO data (id, ${columns}) VALUES (?, ${placeholders})`;
    await this.execute(sqlQuery, [id, ...values]);
    this._changes$.next({ type: "local-upsert", doc, version: Date.now() });
  }

  async delete(id: string): Promise<void> {
    await this.execute("DELETE FROM data WHERE id = ?", [id]);
    this._changes$.next({ type: "local-delete", id, version: Date.now() });
  }

  get state$(): Observable<SyncState> {
    return this._state$.asObservable();
  }

  async sync(_options?: SyncOptions): Promise<void> {
    if (!this.sqlDatabase || this.closed) return;

    this._state$.next({ ...this._state$.value, status: "connecting" });

    try {
      const syncClient = await this.getSyncClient();

      const syncSession = await syncClient.attach({
        identity: this.handle.storeIdentity,
        direction: "Bidirectional",
      });

      const transport = new JsPushSyncTransport({
        send: (data: Uint8Array) => {
          syncSession.send(data);
          return Promise.resolve();
        },
        receive: () => syncSession.receive(),
      });

      await this.sqlDatabase.synchronize(
        transport,
        new PushSynchronizeConfig(),
      );

      this._state$.next({ ...this._state$.value, status: "synced" });
    } catch (error) {
      this._state$.next({
        ...this._state$.value,
        status: "error",
        error: error as Error,
      });
      throw error;
    }
  }

  async close(): Promise<void> {
    this.closed = true;
    this._changes$.complete();
    this._state$.complete();
  }

  private async getSyncClient(): Promise<SyncClient> {
    if (this.syncClient) return this.syncClient;
    this.syncClient = new SyncClient({
      storageApiUrl: this.config.apiUrl,
      tokenProvider: () => this.config.getToken(),
    });
    return this.syncClient;
  }
}

/** Node.js KV store handle */
class NodeKvStoreHandle<T = unknown> implements KvStoreHandle<T> {
  private _changes$ = new Subject<ChangeEvent<T>>();
  private _state$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });

  constructor(
    _handle: { resourceId: string; storeIdentity: StoreIdentity },
    _config: SyncConfig,
  ) {}

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

/** Node.js filesystem handle */
class NodeFilesystemHandle implements FilesystemHandle {
  private _changes$ = new Subject<ChangeEvent<FileEntry>>();
  private _state$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });

  constructor(
    _handle: { resourceId: string; storeIdentity: StoreIdentity },
    _config: SyncConfig,
  ) {}

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

/** Create sync SDK instance for Node.js */
export async function createSync(config: SyncConfig): Promise<SyncSDK> {
  const sdk = new NodeSyncSDK(config);
  return sdk;
}
