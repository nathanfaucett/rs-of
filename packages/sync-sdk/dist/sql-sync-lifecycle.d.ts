import { type Database } from "@ofsdk/sql";
import { type StoreIdentity, type SyncClient } from "@ofsdk/storage-sync-client";
interface LifecycleCallbacks {
    connecting(): void;
    synced(): void;
    failed(error: Error): void;
}
/** Owns one handle's attach, transport, deadline, and close lifecycle. */
export declare class SqlSyncLifecycle {
    private readonly database;
    private readonly client;
    private readonly identity;
    private readonly callbacks;
    private active?;
    private closed;
    constructor(database: Database, client: SyncClient, identity: StoreIdentity, callbacks: LifecycleCallbacks);
    sync(): Promise<void>;
    close(): Promise<void>;
    private run;
}
export {};
//# sourceMappingURL=sql-sync-lifecycle.d.ts.map