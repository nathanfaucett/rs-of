// Browser implementation of @ofsdk/sync-sdk

import { BehaviorSubject, Observable, Subject } from "rxjs";
import { StorageClient } from "@ofsdk/storage-client";
import {
  SyncClient,
  StoreIdentity,
  SyncDirection,
} from "@ofsdk/storage-sync-client";
import {
  SqlDatabase,
  SqlSessionConfig,
  SqlSyncRole,
  JsSqlSyncTransport,
} from "@ofsdk/sql";
import { FileSystemHandle } from "@ofsdk/fs";
import {
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

/** Browser sync SDK implementation */
export class BrowserSyncSDK implements SyncSDK {
  private storageClient: StorageClient;
  private syncClient: SyncClient | null = null;
  private databases = new Map<string, BrowserDatabaseHandle<unknown>>();
  private kvStores = new Map<string, BrowserKvStoreHandle<unknown>>();
  private filesystems = new Map<string, BrowserFilesystemHandle>();
  private _globalState$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });
  public readonly state$ = this._globalState$.asObservable();
  private closed = false;

  constructor(private config: SyncConfig) {
    this.storageClient = new StorageClient({
      idpBaseUrl: config.apiUrl.replace("/storage", "/idp"),
      storageBaseUrl: config.apiUrl,
      clientId: config.appId,
      bearerToken: config.getToken,
    });
  }

  /** Initialize the sync client */
  private async initSyncClient(): Promise<SyncClient> {
    if (this.syncClient) return this.syncClient;

    this.syncClient = new SyncClient({
      storageApiUrl: this.config.apiUrl,
      tokenProvider: () => this.config.getToken(),
    });

    return this.syncClient;
  }

  /** SQL database sync */
  sql = {
    open: async (databaseId: string): Promise<DatabaseHandle> => {
      if (this.databases.has(databaseId)) {
        return this.databases.get(databaseId)!;
      }

      const handle = await this.storageClient.openSqlDatabase(databaseId);
      // Open local WASM database with the resourceId as name
      const sqlDatabase = await SqlDatabase.open(handle.id);
      const storeIdentity: StoreIdentity = {
        storeId: handle.identity.storeId,
        replicaId: handle.identity.replicaId,
        creationEpoch: 0n,
        schemaFingerprint: new Uint8Array(32),
      };
      const dbHandle = new BrowserDatabaseHandle(
        {
          sqlDatabase,
          resourceId: handle.id,
          storeIdentity,
        },
        this.config,
      );
      this.databases.set(databaseId, dbHandle);
      return dbHandle;
    },

    create: async (name: string): Promise<DatabaseHandle> => {
      const handle = await this.storageClient.createSqlDatabase(name);
      const sqlDatabase = await SqlDatabase.open(handle.id);
      const storeIdentity: StoreIdentity = {
        storeId: handle.identity.storeId,
        replicaId: handle.identity.replicaId,
        creationEpoch: 0n,
        schemaFingerprint: new Uint8Array(32),
      };
      const dbHandle = new BrowserDatabaseHandle(
        {
          sqlDatabase,
          resourceId: handle.id,
          storeIdentity,
        },
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
      const kvHandle = new BrowserKvStoreHandle(
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
      const kvHandle = new BrowserKvStoreHandle(
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
      const fsHandle = new BrowserFilesystemHandle(
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
      const fsHandle = new BrowserFilesystemHandle(
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
class BrowserDatabaseHandle<T = unknown> implements DatabaseHandle<T> {
  private _changes$ = new Subject<ChangeEvent<T>>();
  private _state$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });
  private sqlDatabase: SqlDatabase | null = null;
  private syncClient: SyncClient | null = null;
  private closed = false;

  constructor(
    private handle: {
      sqlDatabase: SqlDatabase;
      resourceId: string;
      storeIdentity: StoreIdentity;
    },
    private config: SyncConfig,
  ) {
    this.sqlDatabase = handle.sqlDatabase;
  }

  get id(): string {
    return this.handle.resourceId;
  }

  async execute(sql: string, params?: unknown[]): Promise<QueryResult> {
    if (!this.sqlDatabase) throw new Error("Database closed");
    const client = this.sqlDatabase.client();
    const result = await client.execute(
      sql,
      params ? JSON.stringify(params) : undefined,
    );
    return JSON.parse(result);
  }

  get changes$(): Observable<ChangeEvent<T>> {
    return this._changes$.asObservable();
  }

  async upsert(doc: T): Promise<void> {
    const id = (doc as any).id;
    const keys = Object.keys(doc as object).filter((k) => k !== "id");
    const columns = keys.join(", ");
    const values = keys.map((k) => (doc as any)[k]);
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

  async sync(options?: SyncOptions): Promise<void> {
    if (!this.sqlDatabase || this.closed) return;

    this._state$.next({ ...this._state$.value, status: "connecting" });

    try {
      const syncClient = await this.getSyncClient();

      const syncSession = await syncClient.attach({
        identity: this.handle.storeIdentity,
        direction: "Bidirectional",
      });

      const transport = new JsSqlSyncTransport({
        send: (data: Uint8Array) => {
          syncSession.send(data);
        },
        receive: () => syncSession.receive(),
      });

      const config = new SqlSessionConfig();

      await this.sqlDatabase.synchronize(
        transport,
        config,
        SqlSyncRole.Initiator,
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

/** Browser KV store handle */
class BrowserKvStoreHandle<T = unknown> implements KvStoreHandle<T> {
  private _changes$ = new Subject<ChangeEvent<T>>();
  private _state$ = new BehaviorSubject<SyncState>({
    status: "disconnected",
    version: 0,
    vectorClock: {},
  });
  private closed = false;

  constructor(
    private handle: { resourceId: string; storeIdentity: StoreIdentity },
    private config: SyncConfig,
  ) {}

  async get(key: string): Promise<T | undefined> {
    return undefined;
  }

  async set(key: string, value: T): Promise<void> {
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

  async sync(options?: SyncOptions): Promise<void> {
    this._state$.next({ ...this._state$.value, status: "connecting" });
    this._state$.next({ ...this._state$.value, status: "synced" });
  }

  async close(): Promise<void> {
    this.closed = true;
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
  private fsHandle: FileSystemHandle | null = null;
  private closed = false;

  constructor(
    private handle: { resourceId: string; storeIdentity: StoreIdentity },
    private config: SyncConfig,
  ) {}

  async read(path: string): Promise<Uint8Array> {
    if (!this.fsHandle) throw new Error("Filesystem not initialized");
    return this.fsHandle.read(path);
  }

  async write(path: string, data: Uint8Array): Promise<void> {
    if (!this.fsHandle) throw new Error("Filesystem not initialized");
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

  async delete(path: string): Promise<void> {
    if (!this.fsHandle) throw new Error("Filesystem not initialized");
    await this.fsHandle.delete(path);
    this._changes$.next({ type: "local-delete", id: path });
  }

  async list(path: string): Promise<FileEntry[]> {
    if (!this.fsHandle) throw new Error("Filesystem not initialized");
    return this.fsHandle.list(path);
  }

  get changes$(): Observable<ChangeEvent<FileEntry>> {
    return this._changes$.asObservable();
  }

  get state$(): Observable<SyncState> {
    return this._state$.asObservable();
  }

  async sync(options?: SyncOptions): Promise<void> {
    this._state$.next({ ...this._state$.value, status: "connecting" });
    this._state$.next({ ...this._state$.value, status: "synced" });
  }

  async close(): Promise<void> {
    this.closed = true;
    if (this.fsHandle) {
      this.fsHandle.close();
    }
    this._changes$.complete();
    this._state$.complete();
  }
}

/** Create sync SDK instance for browser */
export async function createSync(config: SyncConfig): Promise<SyncSDK> {
  const sdk = new BrowserSyncSDK(config);
  return sdk;
}
