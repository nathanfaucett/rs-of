// ── Endpoint ────────────────────────────────────────────────────────────────

import type { WasmWebServer } from "./wasm.js";
import { loadWasm } from "./wasm.js";
import type { EndpointId, EndpointOptions, Unsubscribe, PairingOffer } from "./types.js";
import { ClosedError, NotReadyError } from "./types.js";
import { Mesh } from "./mesh.js";

/** A browser mesh endpoint. */
export class Endpoint {
  /** Resolves when the endpoint is connected and ready. */
  readonly ready: Promise<void>;

  /** Mesh communication handle (broadcast, send, onMessage, openStream). */
  readonly mesh: Mesh;

  private server!: WasmWebServer;
  private closed = false;
  private readyResolved = false;

  private pendingListeners: Array<() => void> = [];
  private pendingPeerAdds: EndpointId[] = [];
  private pendingPeerRemoves: EndpointId[] = [];
  private pendingPairingEnabled: boolean | null = null;

  constructor(options: EndpointOptions = {}) {
    this.ready = this.init(options).then(() => {
      this.readyResolved = true;
      this.flushPending();
    });
    // Prevent unhandled rejection if init fails
    this.ready.catch(() => {});

    this.mesh = new Mesh(this.server, this.ready, () => this.closed);
  }

  private async init(options: EndpointOptions): Promise<void> {
    const { WebServer } = await loadWasm();
    this.server = await WebServer.create(
      options.secretKey ?? null,
      options.relayUrls ?? [],
    );
  }

  private assertNotClosed(): void {
    if (this.closed) throw new ClosedError();
  }

  private assertReady(): void {
    if (!this.readyResolved) throw new NotReadyError();
  }

  private flushPending(): void {
    for (const fn of this.pendingListeners) fn();
    this.pendingListeners = [];
    for (const id of this.pendingPeerAdds) this.server.add_peer(id);
    this.pendingPeerAdds = [];
    for (const id of this.pendingPeerRemoves) this.server.remove_peer(id);
    this.pendingPeerRemoves = [];
    if (this.pendingPairingEnabled !== null) {
      this.server.set_pairing_enabled(this.pendingPairingEnabled);
      this.pendingPairingEnabled = null;
    }
  }

  /** This endpoint's ID. Await `ready` first. */
  get id(): EndpointId {
    this.assertNotClosed();
    this.assertReady();
    return this.server.endpoint_id() as EndpointId;
  }

  // peers ──────────────────────────────────────────────────────────────────

  /** Allowlisted peer IDs (inventory, not live connection state).
   * Await `ready` first.
   */
  get peers(): EndpointId[] {
    this.assertNotClosed();
    this.assertReady();
    return this.server.peers() as EndpointId[];
  }

  addPeer(id: EndpointId): void {
    this.assertNotClosed();
    if (this.readyResolved) {
      this.server.add_peer(id);
    } else {
      this.pendingPeerAdds.push(id);
    }
  }

  removePeer(id: EndpointId): void {
    this.assertNotClosed();
    if (this.readyResolved) {
      this.server.remove_peer(id);
    } else {
      this.pendingPeerRemoves.push(id);
    }
  }

  // pairing ──────────────────────────────────────────────────────────────────

  /** Enable or disable accepting pairing offers. */
  set pairingEnabled(enabled: boolean) {
    this.assertNotClosed();
    if (this.readyResolved) {
      this.server.set_pairing_enabled(enabled);
    } else {
      this.pendingPairingEnabled = enabled;
    }
  }

  /** Send a pairing offer. Resolves with the peer's reply. */
  async pair(to: EndpointId, payload: Uint8Array): Promise<Uint8Array> {
    this.assertNotClosed();
    await this.ready;
    return this.server.send_pairing_offer(to, payload);
  }

  /** Listen for incoming pairing offers. Returns an unsubscribe function. */
  onPairingOffer(listener: (offer: PairingOffer) => void): Unsubscribe {
    this.assertNotClosed();
    const register = () => {
      const handle = this.server.subscribe_pairing_offers(
        (offerHandle, remote, payload) => {
          listener({
            remote: remote as EndpointId,
            payload,
            reply: (replyPayload) =>
              this.server.reply_to_pairing_offer(offerHandle, replyPayload),
          });
        },
      );
      return () => this.server.unsubscribe_pairing_offers(handle);
    };
    if (this.readyResolved) return register();
    const unsubscribe = () => {};
    this.pendingListeners.push(() => {
      Object.assign(unsubscribe, register());
    });
    return unsubscribe;
  }

  // lifecycle ────────────────────────────────────────────────────────────────

  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    this.pendingListeners = [];
    this.pendingPeerAdds = [];
    this.pendingPeerRemoves = [];
    this.pendingPairingEnabled = null;
    if (this.server) await this.server.close();
  }
}
