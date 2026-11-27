import { type Database } from "@ofsdk/sql";
import { type StoreIdentity, SyncClient, type SyncClientOptions } from "@ofsdk/storage-sync-client";
interface LifecycleCallbacks {
    connecting(): void;
    synced(): void;
    failed(error: Error): void;
}
export interface SqlResourceSyncOptions extends SyncClientOptions {
    /** Use established only after the app has verified an existing replica binding. */
    mode: "first-upload" | "established";
}
export interface SqlResourceSync {
    /** Exchange SQL history. First upload resolves only after Storage confirms publication. */
    sync(): Promise<void>;
    /** Cancel and await the exchange without closing the caller's database. */
    close(): Promise<void>;
}
/** Bind an already-open database to one Resource without starting sync or taking ownership. */
export declare function createSqlResourceSync(database: Database, identity: StoreIdentity, options: SqlResourceSyncOptions): SqlResourceSync;
/** Owns one handle's attach, transport, deadline, and close lifecycle. */
export declare class SqlSyncLifecycle implements SqlResourceSync {
    private active;
    private closed;
    private readonly database;
    private readonly client;
    private readonly identity;
    private readonly callbacks;
    private readonly mode;
    constructor(database: Database, client: SyncClient, identity: StoreIdentity, callbacks: LifecycleCallbacks | undefined, mode?: SqlResourceSyncOptions["mode"]);
    sync(): Promise<void>;
    close(): Promise<void>;
    private run;
}
export {};
//# sourceMappingURL=sql-sync-lifecycle.d.ts.map