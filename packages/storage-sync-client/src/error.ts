type SyncErrorCode =
  | "auth"
  | "protocol"
  | "transport"
  | "unavailable"
  | "cancelled"
  | "destination-has-history";

export class SyncError extends Error {
  readonly code: SyncErrorCode;
  constructor(code: SyncErrorCode, message: string) {
    super(message);
    this.name = "SyncError";
    this.code = code;
  }
}
export function serverAbort(reason: number): SyncError {
  if (reason === 4)
    return new SyncError("destination-has-history", "Sync destination already has history");
  return new SyncError(
    reason === 0 ? "auth" : reason === 1 ? "unavailable" : "protocol",
    reason === 0
      ? "Sync authorization rejected"
      : reason === 1
        ? "Sync backend unavailable"
        : "Sync server rejected the protocol"
  );
}
