export { ResourcesResidencyApi } from "./generated/apis/ResourcesResidencyApi.js";
export { ResidencyMode } from "./generated/models/ResidencyMode.js";
export type { ResourceResidency } from "./generated/models/ResourceResidency.js";
export { Configuration as StorageOwnerConfiguration } from "./generated/runtime.js";
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
type FetchFunction = (input: URL | RequestInfo, init?: RequestInit) => Promise<Response>;
export type StorageClientOptions = {
    idpBaseUrl: URL | string;
    storageBaseUrl: URL | string;
    clientId: string;
    bearerToken: () => string | Promise<string>;
    fetch?: FetchFunction;
};
export declare class StorageSocket {
    #private;
    readonly socket: WebSocket;
    constructor(socket: WebSocket);
    request(request: StorageRequest): Promise<StorageResponse>;
    close(): void;
}
export declare class StorageClient {
    private readonly options;
    constructor(options: StorageClientOptions);
    listFileSystems(signal?: AbortSignal): Promise<FileSystemResource[]>;
    createFileSystem(name?: string, signal?: AbortSignal): Promise<FileSystemResource>;
    openSocket(filesystemId: string, signal?: AbortSignal): Promise<StorageSocket>;
    private exchangeToken;
    private resources;
}
//# sourceMappingURL=index.d.ts.map