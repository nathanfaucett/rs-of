import { JsSqlSyncTransport } from "@ofsdk/sql";
import { SyncClient, SyncError, } from "@ofsdk/storage-sync-client";
const ATTACH_TIMEOUT_MS = 10_000;
const INACTIVITY_TIMEOUT_MS = 30_000;
/** Bind an already-open database to one Resource without starting sync or taking ownership. */
export function createSqlResourceSync(database, identity, options) {
    const { mode } = options;
    if (mode !== "first-upload" && mode !== "first-restore" && mode !== "established")
        throw new SyncError("protocol", "Unknown SQL Resource sync mode");
    return new SqlSyncLifecycle(database, new SyncClient(options), identity, undefined, mode, options.mode === "first-restore" ? new Uint8Array(options.localTag) : undefined);
}
/** Owns one handle's attach, transport, deadline, and close lifecycle. */
export class SqlSyncLifecycle {
    active;
    closed = false;
    database;
    client;
    identity;
    callbacks;
    mode;
    localTag;
    constructor(database, client, identity, callbacks, mode = "established", localTag) {
        this.database = database;
        this.client = client;
        this.identity = { ...identity, schemaFingerprint: new Uint8Array(identity.schemaFingerprint) };
        this.mode = mode;
        this.localTag = localTag;
        this.callbacks = callbacks;
    }
    sync() {
        if (this.closed)
            return Promise.reject(new SyncError("cancelled", "Sync lifecycle closed"));
        if (this.active)
            return this.active.completion;
        const controller = new AbortController();
        const completion = Promise.resolve().then(() => this.run(controller));
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
        let transportFailure;
        this.callbacks?.connecting();
        try {
            arm("attach");
            const attach = { identity: this.identity, signal: controller.signal };
            const upload = this.mode === "first-upload" ? await this.client.attachFirstUpload(attach) : undefined;
            const attached = upload ??
                (await this.client.attach({
                    ...attach,
                    direction: this.mode === "first-restore" ? "Pull" : "Bidirectional",
                }));
            session = attached;
            if (this.mode === "first-restore" && attached.result.grantedDirection !== "Pull")
                throw new SyncError("protocol", "Storage did not grant a read-only restore");
            arm("inactivity");
            const withActivity = async (operation) => {
                try {
                    const value = await operation();
                    arm("inactivity");
                    return value;
                }
                catch (error) {
                    transportFailure = error;
                    throw error;
                }
            };
            const transport = new JsSqlSyncTransport({
                send: (data) => withActivity(() => attached.send(data)),
                receive: () => withActivity(() => attached.receive()),
            });
            if (this.mode === "first-restore") {
                if (!this.localTag)
                    throw new SyncError("protocol", "Missing local restore tag");
                await this.database.restoreHistory(transport, controller.signal, this.localTag);
            }
            else {
                await this.database.synchronizeHistory(transport);
            }
            if (upload && !controller.signal.aborted)
                await upload.waitForPublish();
            if (this.closed || controller.signal.aborted) {
                throw new SyncError("cancelled", "Sync session cancelled");
            }
            this.callbacks?.synced();
        }
        catch (cause) {
            // WASM wraps errors. Preserve the initial exchange failure reason for callers.
            const sqlCause = cause instanceof Error ? cause.cause : undefined;
            const localHistory = this.mode === "first-restore" &&
                typeof sqlCause === "object" &&
                sqlCause !== null &&
                "destinationHasHistory" in sqlCause &&
                sqlCause.destinationHasHistory === true;
            const failure = localHistory
                ? new SyncError("destination-has-history", "Merge required: this browser has local history. No data was changed.")
                : this.mode !== "established" && transportFailure instanceof SyncError
                    ? transportFailure
                    : cause;
            const error = timeout
                ? new SyncError("transport", timeout === "attach" ? "Sync attach timed out" : "Sync receive timed out")
                : failure instanceof Error
                    ? failure
                    : new Error("SQL synchronization failed");
            this.callbacks?.failed(error);
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