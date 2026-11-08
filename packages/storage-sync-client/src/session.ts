import type { AttachResult } from "./client.js";
import { SyncError, serverAbort } from "./error.js";
import { frame, owner, response, unframe } from "./protocol.js";

export interface SyncSession {
    readonly result: Readonly<AttachResult>;
    send(payload: Uint8Array): void;
    receive(): Promise<Uint8Array>;
    close(): void;
}
export function createSession(
    socket: WebSocket,
    result: Readonly<AttachResult>,
    signal: AbortSignal | undefined,
    usedBytes: bigint,
): SyncSession {
    let ended: SyncError | undefined;
    const queue: Uint8Array[] = [];
    const waiting: {
        resolve: (data: Uint8Array) => void;
        reject: (error: SyncError) => void;
    }[] = [];
    const stop = (error: SyncError, preserveBuffered = false) => {
        if (ended) return;
        ended = error;
        socket.removeEventListener("message", received);
        socket.removeEventListener("error", failed);
        socket.removeEventListener("close", closed);
        signal?.removeEventListener("abort", cancelled);
        if (!preserveBuffered) queue.length = 0;
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
                throw new SyncError(
                    "protocol",
                    "Sync frame exceeds negotiated limit",
                );
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
            if (message.type !== "owner")
                throw new Error("Expected an ofdb SQL owner message");
            const pending = waiting.shift();
            if (pending) pending.resolve(message.data);
            else queue.push(message.data);
        } catch {
            stop(
                new SyncError(
                    "protocol",
                    "Invalid sync session message or byte limit exceeded",
                ),
            );
        }
    };
    const failed = () =>
        stop(new SyncError("transport", "Sync WebSocket failed"));
    const closed = () =>
        stop(new SyncError("transport", "Sync WebSocket closed"), true);
    const cancelled = () =>
        stop(new SyncError("cancelled", "Sync session cancelled"));
    socket.addEventListener("message", received);
    socket.addEventListener("error", failed);
    socket.addEventListener("close", closed);
    signal?.addEventListener("abort", cancelled, { once: true });
    if (signal?.aborted) cancelled();
    if (usedBytes > result.maxSessionBytes)
        stop(
            new SyncError(
                "protocol",
                "Sync session exceeds negotiated byte limit",
            ),
        );
    return {
        result,
        send(payload) {
            if (ended) throw ended;
            let message: Uint8Array;
            try {
                message = owner(payload);
            } catch {
                const error = new SyncError(
                    "protocol",
                    "Sync owner payload exceeds frame limit",
                );
                stop(error);
                throw error;
            }
            send(message);
        },
        receive() {
            const data = queue.shift();
            if (data) return Promise.resolve(data);
            if (ended) return Promise.reject(ended);
            return new Promise((resolve, reject) =>
                waiting.push({ resolve, reject }),
            );
        },
        close() {
            queue.length = 0;
            cancelled();
        },
    };
}
