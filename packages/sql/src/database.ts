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

  /** Release this handle. Independently owned clients remain usable. */
  close(): void {
    this.handle.close();
  }

  /** Create a query handle sharing this database's storage. */
  client(): Client {
    return new Client(this.handle.get().client());
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
   */
  async synchronizeHistory(
    transport: JsSqlSyncTransport,
    role: SqlHistoryRole = SqlHistoryRole.Initiator
  ): Promise<SqlHistoryResult> {
    try {
      return await this.handle.run((wasm) => wasm.synchronize_history(transport, role));
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
