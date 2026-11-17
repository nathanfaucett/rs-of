// ── Mesh ────────────────────────────────────────────────────────────────────

import type { WasmWebServer } from "./wasm.js";
import type { EndpointId, Message, Unsubscribe } from "./types.js";
import { Stream } from "./stream.js";

/** Mesh communication handle. Obtain via `endpoint.mesh`. */
export class Mesh {
  private readonly server: WasmWebServer;
  private readonly ready: Promise<void>;
  private readonly closed: () => boolean;

  constructor(
    server: WasmWebServer,
    ready: Promise<void>,
    closed: () => boolean,
  ) {
    this.server = server;
    this.ready = ready;
    this.closed = closed;
  }

  private assertNotClosed(): void {
    if (this.closed()) throw new Error("endpoint is closed");
  }

  /** Broadcast to all peers in the mesh. */
  async broadcast(payload: Uint8Array): Promise<void> {
    this.assertNotClosed();
    await this.ready;
    await this.server.broadcast(payload);
  }

  /** Send a direct message to one peer.
   *
   * The destination must be in the local peer allowlist (`endpoint.addPeer`),
   * otherwise the call fails with a `not-allowed` error from the mesh.
   * Messages sent to self are delivered to local `onMessage` listeners.
   */
  async send(to: EndpointId, payload: Uint8Array): Promise<void> {
    this.assertNotClosed();
    await this.ready;
    await this.server.send(to, payload);
  }

  /** Listen for incoming messages. Returns an unsubscribe function.
   *
   * Broadcast messages have `destination === null`.
   * Direct messages have `destination === this endpoint's id`.
   * Messages sent to self are delivered locally.
   */
  onMessage(listener: (msg: Message) => void): Unsubscribe {
    this.assertNotClosed();
    const register = () => {
      const handle = this.server.subscribe_messages(
        (source, destination, payload) => {
          listener({
            source: source as EndpointId,
            destination:
              destination === null ? null : (destination as EndpointId),
            payload,
          });
        },
      );
      return () => this.server.unsubscribe_messages(handle);
    };
    // Note: we don't buffer here because Mesh is only available after ready
    return register();
  }

  /** Open a bidirectional byte stream to a peer. */
  async openStream(to: EndpointId): Promise<Stream> {
    this.assertNotClosed();
    await this.ready;
    return new Stream(await this.server.open_stream(to));
  }
}
