<script lang="ts">
    import KvTab from "./KvTab.svelte";
    import {
        createNetworkState,
        ensureState,
        getDefaultKvState,
    } from "./stores/peer-network.js";
    // Required for Vite to bundle WASM - do not change to type-only import
    // biome-ignore lint/correctness/noUnusedImports: Used in generic type parameters and at runtime
    import { type KvPeer, KvPeerNetwork } from "./sync/kv-network.js";

    let kvState = $state(
        createNetworkState<KvPeerNetwork, KvPeer, { value: string }>(
            "Starting…",
        ),
    );

    let kvKey = $state("theme");

    function msg(error: unknown): string {
        return error instanceof Error ? error.message : String(error);
    }

    function kvReport(text: string, error = false): void {
        kvState.status = text;
        kvState.statusError = error;
    }

    async function refreshKvValues(): Promise<void> {
        if (!kvState.network) return;
        for (const peer of kvState.network.getPeers()) {
            const editorState = kvState.editorState[peer.id];
            if (!editorState) continue;
            try {
                const val = await peer.client.get(kvKey);
                editorState.value =
                    val === undefined
                        ? "\u2014"
                        : typeof val === "string"
                          ? val
                          : JSON.stringify(val);
            } catch {
                editorState.value = "error";
            }
        }
    }

    async function initKvNetwork(): Promise<void> {
        try {
            const network = new KvPeerNetwork("ofdb-example-kv");
            network.setListener(() => {
                kvState.peers = network.getPeers();
                for (const peer of kvState.peers) {
                    ensureState(
                        kvState.editorState,
                        peer.id,
                        getDefaultKvState(),
                    );
                }
                void refreshKvValues();
            });
            kvState.network = network;
            await network.addPeer();
            kvState.peers = network.getPeers();
            for (const peer of kvState.peers) {
                ensureState(kvState.editorState, peer.id, getDefaultKvState());
            }
            kvState.busy = false;
            await refreshKvValues();
            kvReport("Ready. Add peers and toggle them offline to test sync.");
        } catch (error) {
            kvReport(`Error: ${msg(error)}`, true);
        }
    }

    $effect(() => {
        initKvNetwork();
    });

    $effect(() => {
        kvKey;
        if (kvState.network && !kvState.busy) {
            void refreshKvValues();
        }
    });

    const kvActions = {
        async addPeer(): Promise<void> {
            if (!kvState.network) return;
            kvState.busy = true;
            try {
                await kvState.network.addPeer();
                kvState.peers = kvState.network.getPeers();
                await refreshKvValues();
                kvReport(`Added peer. Total: ${kvState.peers.length}`);
            } catch (error) {
                kvReport(`Error: ${msg(error)}`, true);
            } finally {
                kvState.busy = false;
            }
        },
        async removePeer(id: string): Promise<void> {
            if (!kvState.network) return;
            await kvState.network.removePeer(id);
            kvState.peers = kvState.network.getPeers();
            delete kvState.editorState[id];
            await refreshKvValues();
        },
        async togglePeer(id: string, online: boolean): Promise<void> {
            if (!kvState.network) return;
            await kvState.network.setOnline(id, online);
            await refreshKvValues();
        },
        async save(peerId: string): Promise<void> {
            if (!kvState.network || kvState.busy) return;
            if (!kvKey.trim()) return;
            kvState.busy = true;
            kvReport("Saving…");
            try {
                const state = kvState.editorState[peerId];
                if (!state) return;
                await kvState.network.write(peerId, kvKey, state.value);
                await refreshKvValues();
                kvReport("Saved (sync runs automatically).");
            } catch (error) {
                kvReport(`Error: ${msg(error)}`, true);
            } finally {
                kvState.busy = false;
            }
        },
        async retrieve(peerId: string): Promise<void> {
            if (kvState.busy) return;
            if (!kvKey.trim()) return;
            kvState.busy = true;
            kvReport("Reading…");
            try {
                const peer = kvState.network?.getPeer(peerId);
                if (peer) {
                    const val = await peer.client.get(kvKey);
                    const editorState = kvState.editorState[peerId];
                    if (editorState) {
                        editorState.value =
                            val === undefined
                                ? "\u2014"
                                : typeof val === "string"
                                  ? val
                                  : JSON.stringify(val);
                    }
                    kvReport("Read from peer.");
                }
            } catch (error) {
                kvReport(`Error: ${msg(error)}`, true);
            } finally {
                kvState.busy = false;
            }
        },
        async refreshValues(): Promise<void> {
            await refreshKvValues();
        },
    };

    import { setKvContext } from "./stores/context.js";

    setKvContext({
        get network() {
            return kvState.network;
        },
        get peers() {
            return kvState.peers;
        },
        get busy() {
            return kvState.busy;
        },
        get status() {
            return kvState.status;
        },
        get statusError() {
            return kvState.statusError;
        },
        get key() {
            return kvKey;
        },
        setKey(key: string) {
            kvKey = key;
        },
        get editorState() {
            return kvState.editorState;
        },
        addPeer: kvActions.addPeer,
        removePeer: kvActions.removePeer,
        togglePeer: kvActions.togglePeer,
        save: kvActions.save,
        retrieve: kvActions.retrieve,
        refreshValues: kvActions.refreshValues,
    });
</script>

<main class="mx-auto max-w-4xl px-4 py-8 font-sans text-base leading-relaxed">
    <h1 class="mb-2 text-2xl font-bold">Browser KV Store with Sync</h1>
    <p class="mb-6 text-gray-600">
        Each peer has its own IndexedDB database. Peers sync automatically when
        online. Toggle a peer offline to simulate losing internet, write on
        another peer, then bring it back online to catch up.
    </p>

    <KvTab />
    <p class="mt-8 text-sm text-gray-500">
        Storage belongs to this browser origin. Peers sync via an in-memory
        transport (MessageChannel-like); replace with WebRTC/WebSocket in
        production.
    </p>
    <noscript>Enable JavaScript and reload to run this Wasm example.</noscript>
</main>
