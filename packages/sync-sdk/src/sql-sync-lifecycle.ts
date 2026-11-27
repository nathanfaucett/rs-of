import { type Database, JsSqlSyncTransport } from "@ofsdk/sql";
import {
  type StoreIdentity,
  SyncClient,
  type SyncClientOptions,
  SyncError,
  type SyncSession,
} from "@ofsdk/storage-sync-client";

const ATTACH_TIMEOUT_MS = 10_000;
const INACTIVITY_TIMEOUT_MS = 30_000;

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
export function createSqlResourceSync(
  database: Database,
  identity: StoreIdentity,
  options: SqlResourceSyncOptions
): SqlResourceSync {
  const { mode } = options;
  if (mode !== "first-upload" && mode !== "established")
    throw new SyncError("protocol", "Unknown SQL Resource sync mode");
  return new SqlSyncLifecycle(database, new SyncClient(options), identity, undefined, mode);
}

interface ActiveSync {
  controller: AbortController;
  completion: Promise<void>;
}

/** Owns one handle's attach, transport, deadline, and close lifecycle. */
export class SqlSyncLifecycle implements SqlResourceSync {
  private active: ActiveSync | undefined;
  private closed: boolean = false;
  private readonly database: Database;
  private readonly client: SyncClient;
  private readonly identity: StoreIdentity;
  private readonly callbacks: LifecycleCallbacks | undefined;
  private readonly mode: SqlResourceSyncOptions["mode"];

  constructor(
    database: Database,
    client: SyncClient,
    identity: StoreIdentity,
    callbacks: LifecycleCallbacks | undefined,
    mode: SqlResourceSyncOptions["mode"] = "established"
  ) {
    this.database = database;
    this.client = client;
    this.identity = { ...identity, schemaFingerprint: new Uint8Array(identity.schemaFingerprint) };
    this.mode = mode;
    this.callbacks = callbacks;
  }

  sync(): Promise<void> {
    if (this.closed) return Promise.reject(new SyncError("cancelled", "Sync lifecycle closed"));
    if (this.active) return this.active.completion;
    const controller = new AbortController();
    const completion = Promise.resolve().then(() => this.run(controller));
    this.active = { controller, completion };
    return completion;
  }

  async close(): Promise<void> {
    this.closed = true;
    const active = this.active;
    if (!active) return;
    active.controller.abort();
    try {
      await active.completion;
    } catch {
      // sync() publishes and throws the failure to its caller; close only awaits it.
    }
  }

  private async run(controller: AbortController): Promise<void> {
    let session: SyncSession | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let timeout: "attach" | "inactivity" | undefined;
    const arm = (kind: "attach" | "inactivity") => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(
        () => {
          timeout = kind;
          controller.abort();
        },
        kind === "attach" ? ATTACH_TIMEOUT_MS : INACTIVITY_TIMEOUT_MS
      );
    };
    let transportFailure: unknown;
    this.callbacks?.connecting();
    try {
      arm("attach");
      const attach = { identity: this.identity, signal: controller.signal };
      const upload =
        this.mode === "first-upload" ? await this.client.attachFirstUpload(attach) : undefined;
      const attached =
        upload ?? (await this.client.attach({ ...attach, direction: "Bidirectional" }));
      session = attached;
      arm("inactivity");
      const withActivity = async <T>(operation: () => T | Promise<T>): Promise<T> => {
        try {
          const value = await operation();
          arm("inactivity");
          return value;
        } catch (error) {
          transportFailure = error;
          throw error;
        }
      };
      const transport = new JsSqlSyncTransport({
        send: (data: Uint8Array) => withActivity(() => attached.send(data)),
        receive: () => withActivity(() => attached.receive()),
      });
      await this.database.synchronizeHistory(transport);
      if (upload && !controller.signal.aborted) await upload.waitForPublish();
      if (this.closed || controller.signal.aborted) {
        throw new SyncError("cancelled", "Sync session cancelled");
      }
      this.callbacks?.synced();
    } catch (cause) {
      // WASM wraps transport failures. Preserve the first-upload abort reason for callers.
      const failure =
        this.mode === "first-upload" && transportFailure instanceof SyncError
          ? transportFailure
          : cause;
      const error = timeout
        ? new SyncError(
            "transport",
            timeout === "attach" ? "Sync attach timed out" : "Sync receive timed out"
          )
        : failure instanceof Error
          ? failure
          : new Error("SQL synchronization failed");
      this.callbacks?.failed(error);
      throw error;
    } finally {
      if (timer) clearTimeout(timer);
      session?.close();
      if (this.active?.controller === controller) this.active = undefined;
    }
  }
}
