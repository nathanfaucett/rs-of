import { getContext, setContext } from "svelte";
import type { SqlPeer, SqlPeerNetwork } from "../sync/sql-network.js";

export interface SqlContextValue {
  network: SqlPeerNetwork | null;
  peers: SqlPeer[];
  busy: boolean;
  status: string;
  statusError: boolean;
  editorState: Record<string, { text: string; records: string[] }>;
  addPeer: () => Promise<void>;
  removePeer: (id: string) => Promise<void>;
  togglePeer: (id: string, online: boolean) => Promise<void>;
  addRecord: (peerId: string) => Promise<void>;
  refreshRecords: () => Promise<void>;
}

const SQL_CTX_KEY = Symbol("sql-context");

export function setSqlContext(value: SqlContextValue): void {
  setContext(SQL_CTX_KEY, value);
}

export function getSqlContext(): SqlContextValue {
  return getContext<SqlContextValue>(SQL_CTX_KEY);
}
