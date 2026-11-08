import type { DatabaseDetailResponse, DatabaseResource, EnsureDatabaseRequest, SqlRequest, SqlResponse } from "./generated/models/index.js";
import type { SyncSessionResponse } from "./generated/models/index.js";
export { ResourcesResidencyApi } from "./generated/apis/ResourcesResidencyApi.js";
export type { DatabaseDetailResponse, DatabaseResource, EnsureDatabaseRequest, SqlRequest, SqlResponse, SqlStatement, SqlValue, } from "./generated/models/index.js";
export { ResidencyMode } from "./generated/models/ResidencyMode.js";
export type { ResourceResidency } from "./generated/models/ResourceResidency.js";
export { Configuration as StorageOwnerConfiguration } from "./generated/runtime.js";
export type StorageAction = "read" | "write";
export declare class DatabaseOperationError extends Error {
    readonly status: number;
    constructor(status: number);
}
export type StorageRequest = {
    type: "read";
    path: string;
} | {
    type: "write";
    path: string;
    content: number[];
} | {
    type: "append";
    path: string;
    content: number[];
} | {
    type: "delete";
    path: string;
} | {
    type: "entry";
    path: string;
} | {
    type: "list";
    path: string;
};
export type StorageEntry = {
    name: string;
    hash: string;
    size: number;
    local: boolean;
};
export type StorageResponse = {
    type: "authenticated";
} | {
    type: "read";
    content: number[];
} | {
    type: "written";
    entry: StorageEntry;
} | {
    type: "appended";
    entry: StorageEntry;
} | {
    type: "deleted";
} | {
    type: "entry";
    entry: StorageEntry;
} | {
    type: "listed";
    entries: StorageEntry[];
} | {
    type: "error";
    code: "invalidRequest" | "operationFailed";
};
export type FileSystemResource = {
    id: string;
    name: string | null;
};
/** SQL Database handle with built-in sync transport */
export declare class SqlDatabaseHandle {
    private readonly storageClient;
    private readonly databaseId;
    private readonly storeIdentity;
    constructor(storageClient: StorageClient, databaseId: string, storeIdentity: {
        storeId: string;
        replicaId: string;
    });
    /** Execute SQL query */
    execute(sql: string, params?: unknown[]): Promise<any>;
    /** Synchronize with remote peer */
    synchronize(): Promise<void>;
    /** Get the database ID */
    get id(): string;
    /** Get the store identity */
    get identity(): {
        storeId: string;
        replicaId: string;
    };
}
/** KV Store handle with built-in sync transport */
export declare class KvStoreHandle {
    private readonly storageClient;
    private readonly storeId;
    private readonly storeIdentity;
    constructor(storageClient: StorageClient, storeId: string, storeIdentity: {
        storeId: string;
        replicaId: string;
    });
    /** Get the store ID */
    get id(): string;
}
/** Filesystem handle with built-in sync transport */
export declare class FilesystemHandle {
    private readonly storageClient;
    private readonly filesystemId;
    private readonly storeIdentity;
    constructor(storageClient: StorageClient, filesystemId: string, storeIdentity: {
        storeId: string;
        replicaId: string;
    });
    /** Open a WebSocket connection for file operations */
    openSocket(signal?: AbortSignal): Promise<StorageSocket>;
    /** Get the filesystem ID */
    get id(): string;
}
type FetchFunction = (input: URL | RequestInfo, init?: RequestInit) => Promise<Response>;
export type StorageClientOptions = {
    idpBaseUrl: URL | string;
    storageBaseUrl: URL | string;
    clientId: string;
    bearerToken: () => string | Promise<string>;
    fetch?: FetchFunction;
};
export type StorageConnection = Readonly<{
    socket: Pick<EventTarget, "addEventListener" | "removeEventListener">;
    request(request: StorageRequest): Promise<StorageResponse>;
    close(): void;
}>;
export declare class StorageSocket {
    #private;
    readonly socket: WebSocket;
    constructor(socket: WebSocket);
    request(request: StorageRequest): Promise<StorageResponse>;
    close(): void;
}
export declare class StorageClient {
    static validateServiceBase(value: URL | string): void;
    private readonly options;
    constructor(options: StorageClientOptions);
    createSqlDatabase(name?: string): Promise<SqlDatabaseHandle>;
    openSqlDatabase(databaseId: string): Promise<SqlDatabaseHandle>;
    listSqlDatabases(signal?: AbortSignal): Promise<DatabaseResource[]>;
    createKvStore(name?: string): Promise<KvStoreHandle>;
    openKvStore(storeId: string): Promise<KvStoreHandle>;
    listKvStores(signal?: AbortSignal): Promise<{
        id: string;
        name: string | null;
    }[]>;
    createFilesystem(name?: string): Promise<FilesystemHandle>;
    openFilesystem(filesystemId: string): Promise<FilesystemHandle>;
    syncToDevice(resourceId: string, targetDeviceId: string): Promise<SyncSessionResponse>;
    ensureDatabase(request: EnsureDatabaseRequest, signal?: AbortSignal): Promise<DatabaseResource>;
    createDatabase(name?: string, signal?: AbortSignal): Promise<DatabaseResource>;
    listDatabases(signal?: AbortSignal): Promise<DatabaseResource[]>;
    getDatabase(databaseId: string, signal?: AbortSignal): Promise<DatabaseDetailResponse>;
    deleteDatabase(databaseId: string, signal?: AbortSignal): Promise<void>;
    executeSql(databaseId: string, request: SqlRequest, actions: StorageAction[], signal?: AbortSignal): Promise<SqlResponse>;
    listFileSystems(signal?: AbortSignal): Promise<FileSystemResource[]>;
    createFileSystem(name?: string, signal?: AbortSignal): Promise<FileSystemResource>;
    openSocket(filesystemId: string, signal?: AbortSignal): Promise<StorageSocket>;
    private exchangeToken;
    private resources;
    private configuration;
}
//# sourceMappingURL=index.d.ts.map