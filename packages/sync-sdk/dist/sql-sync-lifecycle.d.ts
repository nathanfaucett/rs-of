import { type Database } from "@ofsdk/sql";
import { type StoreIdentity, SyncClient, type SyncClientOptions } from "@ofsdk/storage-sync-client";
interface LifecycleCallbacks {
    connecting(): void;
    synced(): void;
    failed(error: Error): void;
}
/** Use established only after the app has verified an existing replica binding. */
export type SqlResourceSyncOptions = SyncClientOptions & ({
    mode: "first-upload" | "established";
} | {
    mode: "first-restore";
    localTag: Uint8Array;
});
export interface SqlResourceSync {
    /** Exchange SQL history. First upload resolves only after Storage confirms publication. */
    sync(): Promise<void>;
    /** Cancel and await the exchange without closing the caller's database. */
    close(): Promise<void>;
}
/** Bind an already-open database to one Resource without starting sync or taking ownership. */
export declare function createSqlResourceSync(database: Database, identity: StoreIdentity, options: SqlResourceSyncOptions): SqlResourceSync;
export type SqlResourceSyncStatus = {
    status: "syncing" | "synced";
} | {
    status: "error";
    error: Error;
};
/**
 * Start commit-driven exchange for an explicitly activated, established binding.
 * The caller owns the database and must close sync on detachment or a session change.
 */
export declare function startSqlResourceSync(database: Database, identity: StoreIdentity, options: SyncClientOptions & {
    onStatus?(state: SqlResourceSyncStatus): void;
}): SqlResourceSync;
/** Owns one handle's attach, transport, deadline, and close lifecycle. */
export declare class SqlSyncLifecycle implements SqlResourceSync {
    private active;
    private closed;
    private readonly database;
    private readonly client;
    private readonly identity;
    private readonly callbacks;
    private readonly mode;
    private readonly localTag;
    constructor(database: Database, client: SyncClient, identity: StoreIdentity, callbacks: LifecycleCallbacks | undefined, mode?: SqlResourceSyncOptions["mode"], localTag?: Uint8Array);
    sync(): Promise<void>;
    close(): Promise<void>;
    private run;
}
export {};
//# sourceMappingURL=sql-sync-lifecycle.d.ts.map