import type { AttachResult } from "./client.js";
import { SyncError, serverAbort } from "./error.js";
import { frame, owner, response, unframe } from "./protocol.js";

export interface SyncSession {
  readonly result: Readonly<AttachResult>;
  /** A failure that invalidates even owner frames already delivered to the consumer. */
  readonly failure?: SyncError | undefined;
  send(payload: Uint8Array): void;
  receive(): Promise<Uint8Array>;
  close(): void;
}
export interface FirstUploadSession extends SyncSession {
  /** Wait for Storage publication after the SQL owner exchange succeeds. */
  waitForPublish(): Promise<void>;
}

type Incoming = Uint8Array | "published";

export function createSession(
  socket: WebSocket,
  result: Readonly<AttachResult>,
  signal: AbortSignal | undefined,
  usedBytes: bigint,
  firstUpload: boolean
): FirstUploadSession {
  let ended: SyncError | undefined;
  let failure: SyncError | undefined;
  const queue: Incoming[] = [];
  const waiting: {
    resolve: (data: Incoming) => void;
    reject: (error: SyncError) => void;
  }[] = [];
  const stop = (error: SyncError, preserveBuffered = false) => {
    if (ended) return;
    ended = error;
    socket.removeEventListener("message", received);
    socket.removeEventListener("error", failed);
    socket.removeEventListener("close", closed);
    signal?.removeEventListener("abort", cancelled);
    if (!preserveBuffered) {
      failure = error;
      queue.length = 0;
    }
    for (const pending of waiting.splice(0)) pending.reject(error);
    socket.close();
  };
  const count = (length: number) => {
    usedBytes += BigInt(length);
    if (usedBytes > result.maxSessionBytes)
      throw new SyncError("protocol", "Sync session exceeds byte limit");
  };
  const send = (payload: Uint8Array) => {
    if (ended) throw ended;
    try {
      if (payload.length > result.maxFrameBytes)
        throw new SyncError("protocol", "Sync frame exceeds negotiated limit");
      const bytes = frame(payload);
      count(bytes.length);
      socket.send(bytes);
    } catch (error) {
      const failure =
        error instanceof SyncError
          ? error
          : new SyncError("transport", "Could not send sync message");
      stop(failure);
      throw failure;
    }
  };
  const received = (event: MessageEvent) => {
    try {
      const payload = unframe(event.data, result.maxFrameBytes);
      count(event.data.byteLength);
      const message = response(payload);
      if (message.type === "abort") {
        stop(serverAbort(message.reason));
        return;
      }
      if (message.type === "ping") {
        send(Uint8Array.of(6));
        return;
      }
      const data =
        message.type === "owner"
          ? message.data
          : firstUpload && message.type === "published"
            ? "published"
            : undefined;
      if (data === undefined) throw new Error("Unexpected sync session message");
      const pending = waiting.shift();
      if (pending) pending.resolve(data);
      else queue.push(data);
      if (data === "published")
        stop(new SyncError("protocol", "First upload receipt already received"), true);
    } catch {
      stop(new SyncError("protocol", "Invalid sync session message or byte limit exceeded"));
    }
  };
  const failed = () => stop(new SyncError("transport", "Sync WebSocket failed"));
  const closed = () => stop(new SyncError("transport", "Sync WebSocket closed"), true);
  const cancelled = () => stop(new SyncError("cancelled", "Sync session cancelled"));
  socket.addEventListener("message", received);
  socket.addEventListener("error", failed);
  socket.addEventListener("close", closed);
  signal?.addEventListener("abort", cancelled, { once: true });
  if (signal?.aborted) cancelled();
  if (usedBytes > result.maxSessionBytes)
    stop(new SyncError("protocol", "Sync session exceeds negotiated byte limit"));
  const next = (): Promise<Incoming> => {
    const data = queue.shift();
    if (data) return Promise.resolve(data);
    if (ended) return Promise.reject(ended);
    return new Promise((resolve, reject) => waiting.push({ resolve, reject }));
  };
  const unexpected = (message: string): never => {
    const error = new SyncError("protocol", message);
    stop(error);
    throw error;
  };
  return {
    result,
    get failure() {
      return failure;
    },
    send(payload) {
      if (ended) throw ended;
      let message: Uint8Array;
      try {
        message = owner(payload);
      } catch {
        const error = new SyncError("protocol", "Sync owner payload exceeds frame limit");
        stop(error);
        throw error;
      }
      send(message);
    },
    async receive() {
      const data = await next();
      if (data === "published") return unexpected("Expected an ofdb SQL owner message");
      return data;
    },
    async waitForPublish() {
      if (!firstUpload) return unexpected("Not a first upload session");
      if ((await next()) !== "published") return unexpected("Expected first upload receipt");
    },
    close() {
      queue.length = 0;
      cancelled();
    },
  };
}
