import { JsSqlSyncTransport } from "@ofsdk/sql";
import { SyncClient, SyncError, } from "@ofsdk/storage-sync-client";
const ATTACH_TIMEOUT_MS = 10_000;
const INACTIVITY_TIMEOUT_MS = 30_000;
/** Bind an already-open database to one Resource without starting sync or taking ownership. */
export function createSqlResourceSync(database, identity, options) {
    const { mode } = options;
    if (mode !== "first-upload" && mode !== "first-restore" && mode !== "established")
        throw new SyncError("protocol", "Unknown SQL Resource sync mode");
    return new SqlSyncLifecycle(database, new SyncClient(options), identity, undefined, mode, options.mode === "first-restore" ? new Uint8Array(options.localTag) : undefined, options.checkActive);
}
/**
 * Start commit-driven exchange for an explicitly activated, established binding.
 * The caller owns the database and must close sync on detachment or a session change.
 */
export function startSqlResourceSync(database, identity, options) {
    const exchange = createSqlResourceSync(database, identity, {
        ...options,
        mode: "established",
    });
    const { onStatus: report, runExclusive } = options;
    const controller = new AbortController();
    let closed = false;
    let pending = false;
    let running;
    async function drain() {
        try {
            while (pending && !closed) {
                pending = false;
                report?.({ status: "syncing" });
                if (closed)
                    break;
                try {
                    if (runExclusive)
                        await runExclusive(() => {
                            controller.signal.throwIfAborted();
                            return exchange.sync();
                        }, controller.signal);
                    else
                        await exchange.sync();
                    if (!closed && !pending)
                        report?.({ status: "synced" });
                }
                catch (cause) {
                    if (!closed) {
                        const error = cause instanceof Error ? cause : new Error("SQL synchronization failed");
                        report?.({ status: "error", error });
                    }
                    if (!pending)
                        throw cause;
                }
            }
        }
        finally {
            running = undefined;
        }
    }
    function sync() {
        if (closed)
            return Promise.reject(new SyncError("cancelled", "Sync lifecycle closed"));
        if (running !== undefined)
            return running;
        pending = true;
        running = Promise.resolve().then(drain);
        return running;
    }
    function wake() {
        pending = true;
        // The SDK reports failure and waits for the next trigger.
        if (running === undefined)
            void sync().catch(() => { });
    }
    const unsubscribe = database.onCommit(wake);
    globalThis.addEventListener?.("online", wake);
    wake();
    return {
        sync,
        async close() {
            if (!closed) {
                closed = true;
                controller.abort();
                unsubscribe();
                globalThis.removeEventListener?.("online", wake);
            }
            await exchange.close();
            await running?.catch(() => { });
        },
    };
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
    checkActive;
    constructor(database, client, identity, callbacks, mode = "established", localTag, checkActive) {
        this.database = database;
        this.client = client;
        this.identity = {
            ...identity,
            schemaFingerprint: new Uint8Array(identity.schemaFingerprint),
        };
        this.mode = mode;
        this.localTag = localTag;
        this.checkActive = checkActive;
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
        let denied = false;
        const checkActive = () => {
            if (session?.failure)
                return false;
            if (!denied && !controller.signal.aborted) {
                try {
                    if (this.checkActive === undefined || this.checkActive() === true)
                        return true;
                }
                catch {
                    // An unreadable fence cannot authorize sync.
                }
            }
            denied = true;
            return false;
        };
        const requireActive = () => {
            if (!checkActive())
                throw session?.failure ?? new SyncError("cancelled", "Sync session cancelled");
        };
        this.callbacks?.connecting();
        try {
            requireActive();
            arm("attach");
            const attach = {
                identity: this.identity,
                signal: controller.signal,
                checkActive,
            };
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
                send: (data) => withActivity(() => {
                    requireActive();
                    attached.send(data);
                }),
                receive: () => withActivity(() => attached.receive()),
            });
            if (this.mode === "first-restore") {
                if (!this.localTag)
                    throw new SyncError("protocol", "Missing local restore tag");
                await this.database.restoreHistory(transport, controller.signal, this.localTag, checkActive);
            }
            else {
                await this.database.synchronizeHistory(transport, undefined, checkActive);
            }
            // Closing must not discard a verified receipt. An unconfirmed upload still rejects.
            if (upload)
                await upload.waitForPublish();
            else
                requireActive();
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
                : (session?.failure ??
                    (this.mode !== "established" && transportFailure instanceof SyncError
                        ? transportFailure
                        : cause));
            const error = timeout
                ? new SyncError("transport", timeout === "attach" ? "Sync attach timed out" : "Sync receive timed out")
                : denied || controller.signal.aborted
                    ? new SyncError("cancelled", "Sync session cancelled")
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