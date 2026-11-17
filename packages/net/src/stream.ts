// ── Stream ────────────────────────────────────────────────────────────────

import type { WasmWebStream } from "./wasm.js";

/** Bidirectional byte stream to a peer. */
export class Stream {
  private readonly inner: WasmWebStream;

  /** @internal Use Mesh.openStream() instead. */
  constructor(inner: WasmWebStream) {
    this.inner = inner;
  }

  write(payload: Uint8Array): Promise<void> {
    return this.inner.write(payload);
  }

  /** Half-close the write side. */
  finish(): void {
    this.inner.finish();
  }

  /** Read everything until the peer finishes, up to `limit` bytes. */
  readToEnd(limit: number): Promise<Uint8Array> {
    return this.inner.read_to_end(limit);
  }
}
