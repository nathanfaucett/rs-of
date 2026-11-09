import { SyncError, serverAbort } from "./error.js";
import {
  attach,
  frame,
  hello,
  MAX_FRAME_BYTES,
  MAX_SESSION_BYTES,
  response,
  type StoreIdentity,
  type SyncDirection,
  unframe,
} from "./protocol.js";
import { createSession, type SyncSession } from "./session.js";

export interface SyncClientOptions {
  storageApiUrl: string;
  tokenProvider: () => string | Promise<string>;
}
export interface AttachOptions {
  identity: StoreIdentity;
  direction: SyncDirection;
  signal?: AbortSignal;
}
export interface AttachResult {
  status: "attached";
  requestedDirection: SyncDirection;
  grantedDirection: SyncDirection;
  backendReplicaId: string;
  sessionId: string;
  maxFrameBytes: number;
  maxSessionBytes: bigint;
}
export class SyncClient {
  private readonly url: string;
  private readonly options: SyncClientOptions;
  constructor(options: SyncClientOptions) {
    this.options = options;
    let url: URL;
    try {
      url = new URL(options.storageApiUrl);
    } catch {
      throw new SyncError("protocol", "Invalid storage API URL");
    }
    if (
      !["http:", "https:"].includes(url.protocol) ||
      url.username ||
      url.password ||
      url.search ||
      url.hash
    )
      throw new SyncError(
        "protocol",
        "Expected an HTTP(S) storage API URL without credentials, query, or fragment",
      );
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    url.pathname = `${url.pathname.replace(/\/+$/, "")}/sync/ws`;
    this.url = url.href;
  }
  attach(options: AttachOptions): Promise<SyncSession> {
    return new Promise((resolve, reject) => {
      let socket: WebSocket | undefined;
      let settled = false;
      const cleanup = () => {
        options.signal?.removeEventListener("abort", cancelled);
        socket?.removeEventListener("open", opened);
        socket?.removeEventListener("message", received);
        socket?.removeEventListener("error", failed);
        socket?.removeEventListener("close", closed);
      };
      const stop = (error: SyncError) => {
        if (settled) return;
        settled = true;
        cleanup();
        socket?.close();
        reject(error);
      };
      const cancelled = () =>
        stop(new SyncError("cancelled", "Sync attach cancelled"));
      const failed = () =>
        stop(new SyncError("transport", "Sync WebSocket failed"));
      const closed = () =>
        stop(new SyncError("transport", "Sync WebSocket closed before attach"));
      let helloPayload: Uint8Array;
      let attachPayload: Uint8Array;
      let echoedHello = false;
      let usedBytes = 0n;
      const send = (payload: Uint8Array) => {
        const bytes = frame(payload);
        usedBytes += BigInt(bytes.length);
        if (usedBytes > MAX_SESSION_BYTES)
          throw new Error("Sync session exceeds byte limit");
        socket?.send(bytes);
      };
      const opened = () => {
        try {
          send(helloPayload);
          send(attachPayload);
        } catch {
          failed();
        }
      };
      const received = (event: MessageEvent) => {
        try {
          const payload = unframe(event.data, MAX_FRAME_BYTES);
          usedBytes += BigInt(event.data.byteLength);
          if (usedBytes > MAX_SESSION_BYTES)
            throw new Error("Sync session exceeds byte limit");
          if (
            !echoedHello &&
            payload.length === helloPayload.length &&
            payload.every((b, i) => b === helloPayload[i])
          ) {
            echoedHello = true;
            return;
          }
          const message = response(payload);
          if (message.type === "ping") {
            send(Uint8Array.of(6));
            return;
          }
          if (message.type === "abort") {
            stop(serverAbort(message.reason));
            return;
          }
          if (message.type !== "attach")
            throw new Error("Expected attach response");
          if (!message.accepted) {
            stop(new SyncError("auth", "Sync attach rejected"));
            return;
          }
          if (
            options.direction === "Bidirectional"
              ? message.grantedDirection !== "Bidirectional"
              : message.grantedDirection !== options.direction &&
                message.grantedDirection !== "Bidirectional"
          )
            throw new Error("Requested sync direction not granted");
          const result: AttachResult = {
            status: "attached",
            requestedDirection: options.direction,
            grantedDirection: message.grantedDirection,
            backendReplicaId: message.backendReplicaId,
            sessionId: message.sessionId,
            maxFrameBytes: Math.min(message.maxFrameBytes, MAX_FRAME_BYTES),
            maxSessionBytes:
              message.maxSessionBytes < MAX_SESSION_BYTES
                ? message.maxSessionBytes
                : MAX_SESSION_BYTES,
          };
          if (usedBytes > result.maxSessionBytes)
            throw new Error("Sync session exceeds negotiated byte limit");
          if (!socket) throw new Error("Missing attached socket");
          cleanup();
          settled = true;
          resolve(
            createSession(
              socket,
              Object.freeze(result),
              options.signal,
              usedBytes,
            ),
          );
        } catch (error) {
          stop(
            new SyncError(
              "protocol",
              error instanceof Error ? error.message : "Invalid sync response",
            ),
          );
        }
      };
      options.signal?.addEventListener("abort", cancelled, {
        once: true,
      });
      if (options.signal?.aborted) {
        cancelled();
        return;
      }
      void (async () => {
        let token: string;
        try {
          token = await this.options.tokenProvider();
        } catch {
          stop(new SyncError("auth", "Could not obtain sync access token"));
          return;
        }
        if (settled) return;
        if (typeof token !== "string" || !token.trim()) {
          stop(new SyncError("auth", "Sync access token is required"));
          return;
        }
        try {
          helloPayload = hello(options.identity);
          attachPayload = attach(options.identity, token, options.direction);
        } catch {
          stop(
            new SyncError(
              "protocol",
              "Invalid sync store identity or direction",
            ),
          );
          return;
        }
        try {
          socket = new WebSocket(this.url);
          socket.binaryType = "arraybuffer";
          socket.addEventListener("open", opened);
          socket.addEventListener("message", received);
          socket.addEventListener("error", failed);
          socket.addEventListener("close", closed);
        } catch {
          failed();
        }
      })();
    });
  }
}
