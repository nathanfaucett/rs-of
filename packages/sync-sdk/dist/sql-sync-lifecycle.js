import { JsSqlSyncTransport } from "@ofsdk/sql";
import { SyncError, } from "@ofsdk/storage-sync-client";
const ATTACH_TIMEOUT_MS = 10_000;
const INACTIVITY_TIMEOUT_MS = 30_000;
/** Owns one handle's attach, transport, deadline, and close lifecycle. */
export class SqlSyncLifecycle {
    active;
    closed = false;
    database;
    client;
    identity;
    callbacks;
    constructor(database, client, identity, callbacks) {
        this.database = database;
        this.client = client;
        this.identity = identity;
        this.callbacks = callbacks;
    }
    sync() {
        if (this.closed)
            return Promise.resolve();
        if (this.active)
            return this.active.completion;
        const controller = new AbortController();
        const completion = this.run(controller);
        this.active = { controller, completion };
        return completion;
    }
    async close() {
        this.closed = true;
        const active = this.active;
        if (!active)
            return;
        active.controller.abort();
        try {
            await active.completion;
        }
        catch {
            // sync() publishes and throws the failure to its caller; close only awaits it.
        }
    }
    async run(controller) {
        let session;
        let timer;
        let timeout;
        const arm = (kind) => {
            if (timer)
                clearTimeout(timer);
            timer = setTimeout(() => {
                timeout = kind;
                controller.abort();
            }, kind === "attach" ? ATTACH_TIMEOUT_MS : INACTIVITY_TIMEOUT_MS);
        };
        this.callbacks.connecting();
        try {
            arm("attach");
            const attached = await this.client.attach({
                identity: this.identity,
                direction: "Bidirectional",
                signal: controller.signal,
            });
            session = attached;
            arm("inactivity");
            const transport = new JsSqlSyncTransport({
                send: (data) => {
                    attached.send(data);
                    arm("inactivity");
                    return Promise.resolve();
                },
                receive: async () => {
                    const data = await attached.receive();
                    arm("inactivity");
                    return data;
                },
            });
            await this.database.synchronizeHistory(transport);
            if (this.closed || controller.signal.aborted) {
                throw new SyncError("cancelled", "Sync session cancelled");
            }
            this.callbacks.synced();
        }
        catch (cause) {
            const error = timeout
                ? new SyncError("transport", timeout === "attach" ? "Sync attach timed out" : "Sync receive timed out")
                : cause instanceof Error
                    ? cause
                    : new Error("SQL synchronization failed");
            this.callbacks.failed(error);
            throw error;
        }
        finally {
            if (timer)
                clearTimeout(timer);
            session?.close();
            if (this.active?.controller === controller)
                this.active = undefined;
        }
    }
}
//# sourceMappingURL=sql-sync-lifecycle.js.map