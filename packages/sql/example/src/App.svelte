<script lang="ts">
// biome-ignore lint/correctness/noUnusedImports: Used in template
import SqlTab from "./SqlTab.svelte";
import { createNetworkState, ensureState, getDefaultSqlState } from "./stores/peer-network.js";
// Required for Vite to bundle WASM - do not change to type-only import
import { type SqlPeer, SqlPeerNetwork } from "./sync/sql-network.js";

let sqlState = $state(
  createNetworkState<SqlPeerNetwork, SqlPeer, { text: string; records: string[] }>("Starting…")
);

function msg(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function sqlReport(text: string, error = false): void {
  sqlState.status = text;
  sqlState.statusError = error;
}

async function refreshSqlRecords(): Promise<void> {
  if (!sqlState.network) return;
  for (const peer of sqlState.network.getPeers()) {
    const state = sqlState.editorState[peer.id];
    if (!state) continue;
    try {
      state.records = await sqlState.network.listRecords(peer);
    } catch {
      state.records = [];
    }
  }
}

async function initSqlNetwork(): Promise<void> {
  try {
    const network = new SqlPeerNetwork("ofdb-example-sql");
    network.setListener(() => {
      sqlState.peers = network.getPeers();
      for (const peer of sqlState.peers) {
        ensureState(sqlState.editorState, peer.id, getDefaultSqlState());
      }
      void refreshSqlRecords();
    });
    sqlState.network = network;
    await network.addPeer();
    sqlState.peers = network.getPeers();
    for (const peer of sqlState.peers) {
      ensureState(sqlState.editorState, peer.id, getDefaultSqlState());
    }
    sqlState.busy = false;
    await refreshSqlRecords();
    sqlReport("Ready. Add peers and toggle them offline to test sync.");
  } catch (error) {
    sqlReport(`Error: ${msg(error)}`, true);
  }
}

$effect(() => {
  initSqlNetwork();
});

const sqlActions = {
  async addPeer(): Promise<void> {
    if (!sqlState.network) return;
    sqlState.busy = true;
    try {
      await sqlState.network.addPeer();
      sqlState.peers = sqlState.network.getPeers();
      await refreshSqlRecords();
      sqlReport(`Added peer. Total: ${sqlState.peers.length}`);
    } catch (error) {
      sqlReport(`Error: ${msg(error)}`, true);
    } finally {
      sqlState.busy = false;
    }
  },
  async removePeer(id: string): Promise<void> {
    if (!sqlState.network) return;
    await sqlState.network.removePeer(id);
    sqlState.peers = sqlState.network.getPeers();
    delete sqlState.editorState[id];
    await refreshSqlRecords();
  },
  async togglePeer(id: string, online: boolean): Promise<void> {
    if (!sqlState.network) return;
    await sqlState.network.setOnline(id, online);
    await refreshSqlRecords();
  },
  async addRecord(peerId: string): Promise<void> {
    if (!sqlState.network || sqlState.busy) return;
    const state = sqlState.editorState[peerId];
    if (!state?.text.trim()) return;
    sqlState.busy = true;
    sqlReport("Adding…");
    try {
      await sqlState.network.addRecord(peerId, state.text);
      state.text = "";
      await refreshSqlRecords();
      sqlReport("Added (sync runs automatically).");
    } catch (error) {
      sqlReport(`Error: ${msg(error)}`, true);
    } finally {
      sqlState.busy = false;
    }
  },
  async refreshRecords(): Promise<void> {
    await refreshSqlRecords();
  },
};

import { setSqlContext } from "./stores/context.js";

setSqlContext({
  get network() {
    return sqlState.network;
  },
  get peers() {
    return sqlState.peers;
  },
  get busy() {
    return sqlState.busy;
  },
  get status() {
    return sqlState.status;
  },
  get statusError() {
    return sqlState.statusError;
  },
  get editorState() {
    return sqlState.editorState;
  },
  addPeer: sqlActions.addPeer,
  removePeer: sqlActions.removePeer,
  togglePeer: sqlActions.togglePeer,
  addRecord: sqlActions.addRecord,
  refreshRecords: sqlActions.refreshRecords,
});
</script>

<main class="mx-auto max-w-4xl px-4 py-8 font-sans text-base leading-relaxed">
    <h1 class="mb-2 text-2xl font-bold">Browser SQL Store with Sync</h1>
    <p class="mb-6 text-gray-600">
        Each peer has its own IndexedDB database with a SQLite backend. Peers
        sync automatically when online. Toggle a peer offline to simulate losing
        internet, write on another peer, then bring it back online to catch up.
    </p>

    <SqlTab />
    <p class="mt-8 text-sm text-gray-500">
        Storage belongs to this browser origin. Peers sync via an in-memory
        transport (MessageChannel-like); replace with WebRTC/WebSocket in
        production.
    </p>
    <noscript>Enable JavaScript and reload to run this Wasm example.</noscript>
</main>
