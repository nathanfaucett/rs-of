import { Database } from "@ofsdk/sql";
import { type StoreIdentity, type SyncClientOptions } from "@ofsdk/storage-sync-client";
import { type Observable } from "rxjs";
import type { DatabaseHandle, FilesystemHandle, KvStoreHandle, SyncConfig, SyncSDK, SyncState } from "./core";
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
    private openOwnedSqlHandle;
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
/**
 * Wrap a caller-owned database for an established replica of the given Storage Resource.
 * This does not open storage, authorize, or start sync. Explicit `sync()` is bidirectional
 * and must not be used for first upload or to join independent histories. `close()`
 * stops this handle's sync without closing the original database.
 */
export declare function createEstablishedSqlHandle(database: Database, identity: StoreIdentity, options: SyncClientOptions): DatabaseHandle;
/** Create sync SDK instance for browser */
export declare function createSync(config: SyncConfig): Promise<SyncSDK>;
//# sourceMappingURL=browser.d.ts.map