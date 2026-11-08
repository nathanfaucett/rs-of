import { EventEmitter } from "events";
import { Observable } from "rxjs";
import { SyncConfig, SyncSDK, DatabaseHandle, KvStoreHandle, FilesystemHandle, SyncState, SyncSession } from "./core";
/** Node.js sync SDK implementation */
export declare class NodeSyncSDK extends EventEmitter implements SyncSDK {
    private config;
    private storageClient;
    private syncClient;
    private databases;
    private kvStores;
    private filesystems;
    private _globalState$;
    readonly state$: Observable<SyncState>;
    private closed;
    constructor(config: SyncConfig);
    /** Initialize the sync client */
    private initSyncClient;
    /** SQL database sync */
    sql: {
        open: (databaseId: string) => Promise<DatabaseHandle>;
        create: (name: string) => Promise<DatabaseHandle>;
        list: () => Promise<{
            id: string;
            name: string | null;
        }[]>;
    };
    /** KV store sync */
    kv: {
        open: (storeId: string) => Promise<KvStoreHandle>;
        create: (name: string) => Promise<KvStoreHandle>;
        list: () => Promise<{
            id: string;
            name: string | null;
        }[]>;
    };
    /** Filesystem sync */
    fs: {
        open: (fsId: string) => Promise<FilesystemHandle>;
        create: (name: string) => Promise<FilesystemHandle>;
        list: () => Promise<{
            id: string;
            name: string | null;
        }[]>;
    };
    syncToDevice(resourceId: string, targetDeviceId: string): Promise<SyncSession>;
    close(): Promise<void>;
}
/** Create sync SDK instance for Node.js */
export declare function createSync(config: SyncConfig): Promise<SyncSDK>;
//# sourceMappingURL=node.d.ts.map