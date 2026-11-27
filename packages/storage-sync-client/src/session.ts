import type { AttachResult } from "./client.js";
import { SyncError, serverAbort } from "./error.js";
import { type AttachTag, frame, owner, response, unframe } from "./protocol.js";

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

export interface RevisionedPullSession extends SyncSession {
  waitForSnapshotRevision(): Promise<Uint8Array>;
}
export interface ComparePublishSession extends SyncSession {
  /** Resolves only for the exact committed operation, not on SQL completion. */
  waitForPublish(): Promise<void>;
}

type Control = Extract<
  ReturnType<typeof response>,
  { type: "published" | "snapshot" | "compare-published" | "publication-status" }
>;
type Incoming = Uint8Array | Control;

export function createSession(
  socket: WebSocket,
  result: Readonly<AttachResult>,
  signal: AbortSignal | undefined,
  usedBytes: bigint,
  intent: AttachTag,
  publication?: Uint8Array
) {
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
        message.type === "owner" && intent !== 13
          ? message.data
          : (intent === 7 && message.type === "published") ||
              (intent === 9 && message.type === "snapshot") ||
              (intent === 11 &&
                message.type === "compare-published" &&
                publication?.every((b, i) => b === message.publication[i])) ||
              (intent === 13 && message.type === "publication-status")
            ? message
            : undefined;
      if (data === undefined) throw new Error("Unexpected sync session message");
      const pending = waiting.shift();
      if (pending) pending.resolve(data);
      else queue.push(data);
      if (!(data instanceof Uint8Array))
        stop(new SyncError("protocol", "Sync control response already received"), true);
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
    send(payload: Uint8Array) {
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
      if (!(data instanceof Uint8Array)) return unexpected("Expected an ofdb SQL owner message");
      return data;
    },
    async waitForPublish() {
      if (intent !== 7 && intent !== 11) return unexpected("Not a publication session");
      const data = await next();
      if (
        data instanceof Uint8Array ||
        (data.type !== "published" && data.type !== "compare-published")
      )
        return unexpected("Expected a publish receipt");
    },
    async waitForSnapshotRevision() {
      if (intent !== 9) return unexpected("Not a revisioned Pull session");
      const data = await next();
      if (data instanceof Uint8Array || data.type !== "snapshot")
        return unexpected("Expected a snapshot revision");
      return data.revision;
    },
    async publicationStatus() {
      if (intent !== 13) return unexpected("Not a receipt lookup session");
      const data = await next();
      if (data instanceof Uint8Array || data.type !== "publication-status")
        return unexpected("Expected a publication status");
      return data.published;
    },
    close() {
      queue.length = 0;
      cancelled();
    },
  };
}
