import { EventEmitter } from "node:events";
import { type Observable } from "rxjs";
import type { DatabaseHandle, FilesystemHandle, KvStoreHandle, SyncConfig, SyncSDK, SyncState } from "./core";
/** Node.js sync SDK implementation */
export declare class NodeSyncSDK extends EventEmitter implements SyncSDK {
    private storageClient;
    private config;
    private databases;
    private kvStores;
    private filesystems;
    private _globalState$;
    readonly state$: Observable<SyncState>;
    constructor(config: SyncConfig);
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
    close(): Promise<void>;
}
/** Create sync SDK instance for Node.js */
export declare function createSync(config: SyncConfig): Promise<SyncSDK>;
//# sourceMappingURL=node.d.ts.map