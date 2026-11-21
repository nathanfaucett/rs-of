import { type Database, JsSqlSyncTransport } from "@ofsdk/sql";
import {
  SyncError,
  type StoreIdentity,
  type SyncSession,
  type SyncClient,
} from "@ofsdk/storage-sync-client";

const ATTACH_TIMEOUT_MS = 10_000;
const INACTIVITY_TIMEOUT_MS = 30_000;

interface LifecycleCallbacks {
  connecting(): void;
  synced(): void;
  failed(error: Error): void;
}

interface ActiveSync {
  controller: AbortController;
  completion: Promise<void>;
}

/** Owns one handle's attach, transport, deadline, and close lifecycle. */
export class SqlSyncLifecycle {
  private active?: ActiveSync;
  private closed = false;

  constructor(
    private readonly database: Database,
    private readonly client: SyncClient,
    private readonly identity: StoreIdentity,
    private readonly callbacks: LifecycleCallbacks,
  ) {}

  sync(): Promise<void> {
    if (this.closed) return Promise.resolve();
    if (this.active) return this.active.completion;
    const controller = new AbortController();
    const completion = this.run(controller);
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
      timer = setTimeout(() => {
        timeout = kind;
        controller.abort();
      }, kind === "attach" ? ATTACH_TIMEOUT_MS : INACTIVITY_TIMEOUT_MS);
    };
    this.callbacks.connecting();
    try {
      arm("attach");
      session = await this.client.attach({
        identity: this.identity,
        direction: "Bidirectional",
        signal: controller.signal,
      });
      arm("inactivity");
      const transport = new JsSqlSyncTransport({
        send: (data: Uint8Array) => {
          session?.send(data);
          arm("inactivity");
          return Promise.resolve();
        },
        receive: async () => {
          const data = await session?.receive();
          arm("inactivity");
          if (!data) throw new SyncError("transport", "Sync receive returned no frame");
          return data;
        },
      });
      await this.database.synchronizeHistory(transport);
      if (this.closed || controller.signal.aborted) {
        throw new SyncError("cancelled", "Sync session cancelled");
      }
      this.callbacks.synced();
    } catch (cause) {
      const error = timeout
        ? new SyncError(
            "transport",
            timeout === "attach"
              ? "Sync attach timed out"
              : "Sync receive timed out",
          )
        : cause;
      this.callbacks.failed(error as Error);
      throw error;
    } finally {
      if (timer) clearTimeout(timer);
      session?.close();
      if (this.active?.controller === controller) this.active = undefined;
    }
  }
}
