import {
  JsPushSyncTransport,
  JsSqlSyncError,
  JsSqlSyncTransport,
  PushConfig,
  PushSynchronizeResult,
  SqlHistoryResult,
  SqlHistoryRole,
  SqlSessionConfig,
  SqlSyncRole,
} from "../pkg/ofdb_wasm.js";
import { Client } from "./client.js";
import { Error as SqlError, wrapError } from "./error.js";
import { Handle } from "./handle.js";
import type { WasmSqlDatabase } from "./wasm.js";
import { loadWasm } from "./wasm.js";

/** Record policy for a private initial-history stage. */
export interface HistoryRecord {
  readonly table: string;
  readonly identity: string;
  /** Additional non-null fields that never change, such as the Vault ID on an item. */
  readonly stable?: readonly string[];
  /** Fields that must come from one live source together. */
  readonly envelope: readonly string[];
}

/** Embedded SQL database handle. Opens the store and creates query handles. */
export class Database {
  private readonly handle: Handle<WasmSqlDatabase>;

  private constructor(wasm: WasmSqlDatabase) {
    this.handle = new Handle(wasm);
  }

  /** Open a persistent SQL database backed by IndexedDB. */
  static async open(name: string): Promise<Database> {
    try {
      const { SqlDatabase: WasmSqlDatabase } = await loadWasm();
      const wasm = await WasmSqlDatabase.open(name);
      return new Database(wasm);
    } catch (error) {
      throw wrapError(error, SqlError);
    }
  }

  /** Open a disposable database with no persistent replica identity. */
  static async inMemory(): Promise<Database> {
    const { SqlDatabase: WasmSqlDatabase } = await loadWasm();
    return new Database(WasmSqlDatabase.in_memory());
  }

  /** Release this handle. Independently owned clients remain usable. */
  close(): void {
    this.handle.close();
  }

  /** Create a query handle sharing this database's storage. */
  client(): Client {
    return new Client(this.handle.get().client());
  }

  /**
   * Observe coalesced durable changes to this IndexedDB store in the current WASM runtime.
   * Includes incoming history applied locally, not remote-only changes or writes in other tabs.
   * Returns a function to unsubscribe. No events are replayed at subscription time.
   */
  onCommit(listener: () => void): () => void {
    let subscribed = true;
    const watch = this.handle.get().watch_commits(() => {
      if (subscribed) listener();
    });
    return () => {
      if (!subscribed) return;
      subscribed = false;
      watch.free();
    };
  }

  /** Synchronize with a remote peer using the given transport. */
  async synchronize(
    transport: JsPushSyncTransport,
    config: SqlSessionConfig
  ): Promise<PushSynchronizeResult> {
    try {
      return await this.handle.run((wasm) => wasm.synchronize(transport, config));
    } catch (error) {
      throw wrapError(error, SqlError);
    }
  }

  /**
   * Exchange SQL history v7 over caller-supplied byte frames.
   * Transports must bound reads and queued frames before buffering.
   * A supplied synchronous `canApply` must return true before each local batch
   * and its commit. A commit already issued to IndexedDB can finish.
   */
  async synchronizeHistory(
    transport: JsSqlSyncTransport,
    role: SqlHistoryRole = SqlHistoryRole.Initiator,
    canApply?: () => boolean
  ): Promise<SqlHistoryResult> {
    try {
      return await this.handle.run((wasm) => wasm.synchronize_history(transport, role, canApply));
    } catch (error) {
      throw wrapError(error, SqlError);
    }
  }

  /** Reconcile two snapshots in a disposable in-memory database. Does not write either source. */
  async stageHistory(other: Database, records: readonly HistoryRecord[]): Promise<Database> {
    try {
      const policy = JSON.stringify(
        records.map(({ table, identity, stable, envelope }) => [
          table,
          identity,
          stable ?? [],
          envelope,
        ])
      );
      return await this.handle.run((left) =>
        other.handle.run(async (right) => new Database(await left.stage_history(right, policy)))
      );
    } catch (error) {
      throw wrapError(error, SqlError);
    }
  }

  /** Capture one bounded history revision, including retained tombstones. */
  async historyRevision(): Promise<Uint8Array> {
    try {
      return await this.handle.run((wasm) => wasm.history_revision());
    } catch (error) {
      throw wrapError(error, SqlError);
    }
  }

  /** Apply the exact private candidate and save the first replica tag in one local commit. */
  async publishLocalHistory(
    staged: Database,
    expected: Uint8Array,
    candidate: Uint8Array,
    tag: Uint8Array,
    canApply?: () => boolean
  ): Promise<void> {
    try {
      return await this.handle.run((wasm) =>
        staged.handle.run((stage) =>
          wasm.publish_local_history(stage, expected, candidate, tag, canApply)
        )
      );
    } catch (error) {
      throw wrapError(error, SqlError);
    }
  }

  /** Preflight only. The restore transaction rechecks retained history. */
  async initialRestoreIsEligible(): Promise<boolean> {
    try {
      return await this.handle.run((wasm) => wasm.initial_restore_is_eligible());
    } catch (error) {
      throw wrapError(error, SqlError);
    }
  }

  async initialRestoreTag(): Promise<Uint8Array | undefined> {
    try {
      return await this.handle.run((wasm) => wasm.initial_restore_tag());
    } catch (error) {
      throw wrapError(error, SqlError);
    }
  }

  /** Replace the local-only replica tag if its current bytes match `expected`. */
  async replaceInitialRestoreTag(
    expected: Uint8Array | undefined,
    tag: Uint8Array
  ): Promise<boolean> {
    try {
      return await this.handle.run((wasm) => wasm.replace_initial_restore_tag(expected, tag));
    } catch (error) {
      throw wrapError(error, SqlError);
    }
  }

  /** Receive into a transient stage and publish only into a pristine database. */
  async restoreHistory(
    transport: JsSqlSyncTransport,
    signal: AbortSignal,
    tag: Uint8Array,
    canApply?: () => boolean
  ): Promise<void> {
    try {
      return await this.handle.run((wasm) =>
        wasm.restore_history(transport, signal, tag, canApply)
      );
    } catch (error) {
      throw wrapError(error, SqlError);
    }
  }
}

export { SqlSyncDirection } from "../pkg/ofdb_wasm.js";
// Re-export sync types and values
export {
  JsPushSyncTransport,
  JsSqlSyncError,
  JsSqlSyncTransport,
  PushConfig,
  PushSynchronizeResult,
  SqlHistoryResult,
  SqlHistoryRole,
  SqlSessionConfig,
  SqlSyncRole,
};
