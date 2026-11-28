type SyncErrorCode =
  | "auth"
  | "protocol"
  | "transport"
  | "unavailable"
  | "cancelled"
  | "destination-has-history"
  | "destination-changed"
  | "publication-already-used";

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
  if (reason === 5)
    return new SyncError("destination-changed", "Sync destination revision changed");
  if (reason === 6)
    return new SyncError("publication-already-used", "Check the original publication receipt");
  return new SyncError(
    reason === 0 ? "auth" : reason === 1 ? "unavailable" : "protocol",
    reason === 0
      ? "Sync authorization rejected"
      : reason === 1
        ? "Sync backend unavailable"
        : "Sync server rejected the protocol"
  );
}
