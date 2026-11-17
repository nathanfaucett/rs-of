// ── Public types ────────────────────────────────────────────────────────────

/** Branded endpoint identifier (hex-encoded public key). */
export type EndpointId = string & { readonly __brand: "EndpointId" };

/** An incoming mesh message. `destination` is null for broadcasts. */
export interface Message {
  readonly source: EndpointId;
  readonly destination: EndpointId | null;
  readonly payload: Uint8Array;
}

/** An incoming pairing offer. Call `reply()` to respond. */
export interface PairingOffer {
  readonly remote: EndpointId;
  readonly payload: Uint8Array;
  reply(payload: Uint8Array): Promise<void>;
}

export interface EndpointOptions {
  /** Hex secret key for a stable identity. Random if omitted. */
  secretKey?: string;
  /** Relay URLs. Defaults to public n0 relays. */
  relayUrls?: string[];
}

/** Returns an unsubscribe function. */
export type Unsubscribe = () => void;

// ── Error types ──────────────────────────────────────────────────────────────

/** Error thrown when an operation is performed on a closed endpoint. */
export class ClosedError extends Error {
  constructor() {
    super("endpoint is closed");
    this.name = "ClosedError";
  }
}

/** Error thrown when accessing an endpoint before ready resolves. */
export class NotReadyError extends Error {
  constructor() {
    super("endpoint not ready — await ready first");
    this.name = "NotReadyError";
  }
}
