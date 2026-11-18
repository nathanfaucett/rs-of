import { getContext, setContext } from "svelte";
import type { KvPeer, KvPeerNetwork } from "../sync/kv-network.js";

export interface KvContextValue {
  network: KvPeerNetwork | null;
  peers: KvPeer[];
  busy: boolean;
  status: string;
  statusError: boolean;
  key: string;
  setKey: (key: string) => void;
  editorState: Record<string, { value: string }>;
  addPeer: () => Promise<void>;
  removePeer: (id: string) => Promise<void>;
  togglePeer: (id: string, online: boolean) => Promise<void>;
  save: (peerId: string) => Promise<void>;
  retrieve: (peerId: string) => Promise<void>;
  refreshValues: () => Promise<void>;
}

const KV_CTX_KEY = Symbol("kv-context");

export function setKvContext(value: KvContextValue): void {
  setContext(KV_CTX_KEY, value);
}

export function getKvContext(): KvContextValue {
  return getContext<KvContextValue>(KV_CTX_KEY);
}
