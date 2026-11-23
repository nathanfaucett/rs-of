/**
 * @ofsdk/net — mesh networking in the browser via ofnet WASM.
 *
 * ```ts
 * import { Endpoint } from "@ofsdk/net";
 *
 * const ep = new Endpoint();
 * await ep.ready;
 * ep.mesh.onMessage((msg) => console.log(new TextDecoder().decode(msg.payload)));
 * await ep.mesh.broadcast(new TextEncoder().encode("hi"));
 * ```
 */

export { Endpoint } from "./endpoint.js";
export { Mesh } from "./mesh.js";

export { Stream } from "./stream.js";
export type {
  EndpointId,
  EndpointOptions,
  Message,
  PairingOffer,
  Unsubscribe,
} from "./types.js";
export { ClosedError, NotReadyError } from "./types.js";
