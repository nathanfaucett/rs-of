/** Sync configuration */
export interface SyncConfig {
    /** Storage service API URL */
    apiUrl: string;
    /** Application ID */
    appId: string;
    /** User ID */
    userId: string;
    /** Function to get sync access token (OIDC) */
    getToken: () => Promise<string>;
    /** Function to get Storage capability token for a resource and actions */
    getStorageToken: (resource: string, actions: ("read" | "write")[]) => Promise<string>;
}
/** Change event types */
export type ChangeEventType = "local-upsert" | "local-delete" | "remote-upsert" | "remote-delete" | "ack" | "conflict";
/** Change event */
export interface ChangeEvent<T = unknown> {
    /** Type of change */
    type: ChangeEventType;
    /** Document (for upsert events) */
    doc?: T;
    /** Document ID (for delete events) */
    id?: string;
    /** Version number */
    version?: number;
    /** Vector clock for causality tracking */
    vectorClock?: Record<string, number>;
    /** Conflict data (for conflict events) */
    conflict?: {
        local: T;
        remote: T;
        base: T;
        resolve: (merged: T) => void;
    };
}
/** Sync state */
export interface SyncState {
    /** Current sync status */
    status: "connecting" | "synced" | "disconnected" | "error";
    /** Current version */
    version: number;
    /** Vector clock */
    vectorClock: Record<string, number>;
    /** Last error if any */
    error?: Error;
}
/** Sync options */
export interface SyncOptions {
    /** Sync direction */
    direction?: "bidirectional" | "push" | "pull";
    /** Conflict resolution strategy */
    conflictStrategy?: "local-wins" | "remote-wins" | "merge" | "manual";
}
/** Database handle interface */
export interface DatabaseHandle<T = unknown> {
    /** Execute SQL query */
    execute(sql: string, params?: unknown[]): Promise<QueryResult>;
    /** Observable stream of changes */
    readonly changes$: import("rxjs").Observable<ChangeEvent<T>>;
    /** Upsert a document */
    upsert(doc: T): Promise<void>;
    /** Delete a document by ID */
    delete(id: string): Promise<void>;
    /** Get current sync state */
    readonly state$: import("rxjs").Observable<SyncState>;
    /** Start sync */
    sync(options?: SyncOptions): Promise<void>;
    /** Close the database */
    close(): Promise<void>;
    /** Get the database ID */
    readonly id: string;
}
/** Query result */
export interface QueryResult {
    columns: string[];
    rows: unknown[][];
}
/** KV store handle interface */
export interface KvStoreHandle<T = unknown> {
    /** Get a value */
    get(key: string): Promise<T | undefined>;
    /** Set a value */
    set(key: string, value: T): Promise<void>;
    /** Delete a value */
    delete(key: string): Promise<void>;
    /** Observable stream of changes */
    readonly changes$: import("rxjs").Observable<ChangeEvent<T>>;
    /** Get current sync state */
    readonly state$: import("rxjs").Observable<SyncState>;
    /** Start sync */
    sync(options?: SyncOptions): Promise<void>;
    /** Close the store */
    close(): Promise<void>;
}
/** Filesystem handle interface */
export interface FilesystemHandle {
    /** Read a file */
    read(path: string): Promise<Uint8Array>;
    /** Write a file */
    write(path: string, data: Uint8Array): Promise<void>;
    /** Delete a file */
    delete(path: string): Promise<void>;
    /** List directory */
    list(path: string): Promise<FileEntry[]>;
    /** Observable stream of changes */
    readonly changes$: import("rxjs").Observable<ChangeEvent<FileEntry>>;
    /** Get current sync state */
    readonly state$: import("rxjs").Observable<SyncState>;
    /** Start sync */
    sync(options?: SyncOptions): Promise<void>;
    /** Close the filesystem */
    close(): Promise<void>;
}
/** File entry */
export interface FileEntry {
    path: string;
    isDirectory: boolean;
    size: number;
    modified: number;
}
/** Main sync SDK interface */
export interface SyncSDK {
    /** SQL database sync */
    sql: {
        /** Open a database by ID */
        open(databaseId: string): Promise<DatabaseHandle>;
        /** Create a new database */
        create(name: string): Promise<DatabaseHandle>;
        /** List databases */
        list(): Promise<{
            id: string;
            name: string | null;
        }[]>;
    };
    /** KV store sync */
    kv: {
        /** Open a KV store by ID */
        open(storeId: string): Promise<KvStoreHandle>;
        /** Create a new KV store */
        create(name: string): Promise<KvStoreHandle>;
        /** List KV stores */
        list(): Promise<{
            id: string;
            name: string | null;
        }[]>;
    };
    /** Filesystem sync */
    fs: {
        /** Open a filesystem by ID */
        open(fsId: string): Promise<FilesystemHandle>;
        /** Create a new filesystem */
        create(name: string): Promise<FilesystemHandle>;
        /** List filesystems */
        list(): Promise<{
            id: string;
            name: string | null;
        }[]>;
    };
    /** Sync a resource to a target device */
    syncToDevice(resourceId: string, targetDeviceId: string): Promise<SyncSession>;
    /** Close all connections */
    close(): Promise<void>;
}
/** Sync session */
export interface SyncSession {
    sessionId: string;
    resourceId: string;
    targetDevice: string;
    status: "connecting" | "synchronizing" | "synced" | "disconnected" | "error";
}
//# sourceMappingURL=core.d.ts.map