export class SyncError extends Error {
    constructor(
        readonly code:
            | "auth"
            | "protocol"
            | "transport"
            | "unavailable"
            | "cancelled",
        message: string,
    ) {
        super(message);
        this.name = "SyncError";
    }
}
export function serverAbort(reason: number): SyncError {
    return new SyncError(
        reason === 0 ? "auth" : reason === 1 ? "unavailable" : "protocol",
        reason === 0
            ? "Sync authorization rejected"
            : reason === 1
              ? "Sync backend unavailable"
              : "Sync server rejected the protocol",
    );
}
