import { type Observable } from "rxjs";
import type { SyncConfig, SyncSDK, DatabaseHandle, KvStoreHandle, FilesystemHandle, SyncState, SyncSession } from "./core";
/** Browser sync SDK implementation */
export declare class BrowserSyncSDK implements SyncSDK {
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
    syncToDevice(resourceId: string, targetDeviceId: string): Promise<SyncSession>;
    close(): Promise<void>;
}
/** Create sync SDK instance for browser */
export declare function createSync(config: SyncConfig): Promise<SyncSDK>;
//# sourceMappingURL=browser.d.ts.map