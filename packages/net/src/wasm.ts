// ── WASM bindings (internal) ────────────────────────────────────────────────

interface WasmModule {
  default(): Promise<void>;
  WebServer: {
    create(
      secretKeyHex: string | null,
      relayUrls: string[],
    ): Promise<WasmWebServer>;
  };
}

interface WasmWebServer {
  endpoint_id(): string;
  broadcast(payload: Uint8Array): Promise<void>;
  send(destinationId: string, payload: Uint8Array): Promise<void>;
  subscribe_messages(
    cb: (
      source: string,
      destination: string | null,
      payload: Uint8Array,
    ) => void,
  ): number;
  unsubscribe_messages(handle: number): void;
  peers(): string[];
  add_peer(id: string): void;
  remove_peer(id: string): void;
  set_pairing_enabled(enabled: boolean): void;
  subscribe_pairing_offers(
    cb: (handle: number, remote: string, payload: Uint8Array) => void,
  ): number;
  unsubscribe_pairing_offers(handle: number): void;
  send_pairing_offer(dest: string, payload: Uint8Array): Promise<Uint8Array>;
  reply_to_pairing_offer(handle: number, payload: Uint8Array): Promise<void>;
  open_stream(peerId: string): Promise<WasmWebStream>;
  close(): Promise<void>;
}

interface WasmWebStream {
  write(payload: Uint8Array): Promise<void>;
  finish(): void;
  read_to_end(limit: number): Promise<Uint8Array>;
}

let wasmModule: Promise<WasmModule> | undefined;

async function loadWasm(): Promise<WasmModule> {
  if (!wasmModule) {
    wasmModule = import("../pkg/iroh_chain_wasm.js")
      .then(async (m) => {
        await m.default();
        return m as unknown as WasmModule;
      })
      .catch((err) => {
        wasmModule = undefined;
        throw err;
      });
  }
  return wasmModule;
}

export type { WasmModule, WasmWebServer, WasmWebStream };
export { loadWasm };
