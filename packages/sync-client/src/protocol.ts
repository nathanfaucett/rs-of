export type SyncDirection = "Push" | "Pull" | "Bidirectional";
export interface StoreIdentity {
  storeId: string;
  replicaId: string;
  creationEpoch: bigint;
  schemaFingerprint: Uint8Array;
}
export const MAX_FRAME_BYTES = 16 * 1024 * 1024;
export const MAX_SESSION_BYTES = 1024n * 1024n * 1024n;
const directions: SyncDirection[] = ["Push", "Pull", "Bidirectional"];

function uint(value: bigint): number[] {
  if (value < 0n || value > 0xffffffffffffffffn)
    throw new Error("Invalid unsigned integer");
  const bytes: number[] = [];
  do {
    const byte = Number(value & 127n);
    value >>= 7n;
    bytes.push(byte | (value ? 128 : 0));
  } while (value);
  return bytes;
}
function vector(bytes: Uint8Array): number[] {
  return [...uint(BigInt(bytes.length)), ...bytes];
}
function uuid(value: string): number[] {
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      value,
    )
  )
    throw new Error("Invalid store UUID");
  return [
    16,
    ...Uint8Array.from(value.replaceAll("-", "").match(/../g) ?? [], (b) =>
      Number.parseInt(b, 16),
    ),
  ];
}
function store(identity: StoreIdentity): number[] {
  if (
    !(identity.schemaFingerprint instanceof Uint8Array) ||
    identity.schemaFingerprint.length !== 32
  )
    throw new Error("Expected a 32-byte schema fingerprint");
  return [
    ...uuid(identity.storeId),
    ...uuid(identity.replicaId),
    ...uint(identity.creationEpoch),
    ...identity.schemaFingerprint,
  ];
}
export function hello(identity: StoreIdentity): Uint8Array {
  return Uint8Array.from([0, 1, 0, ...store(identity), 1, 0]);
}
export function attach(
  identity: StoreIdentity,
  token: string,
  direction: SyncDirection,
): Uint8Array {
  const tag = directions.indexOf(direction);
  if (tag < 0) throw new Error("Invalid sync direction");
  const identityBytes = store(identity);
  const tokenBytes = new TextEncoder().encode(token);
  if (
    2 +
      identityBytes.length +
      uint(BigInt(tokenBytes.length)).length +
      tokenBytes.length >
    MAX_FRAME_BYTES
  )
    throw new Error("Sync attach exceeds frame limit");
  return Uint8Array.from([1, ...identityBytes, ...vector(tokenBytes), tag]);
}
export function owner(payload: Uint8Array): Uint8Array {
  if (
    payload.length + uint(BigInt(payload.length)).length + 2 >
    MAX_FRAME_BYTES
  )
    throw new Error("Sync frame exceeds limit");
  const prefix = Uint8Array.from([3, 0, ...uint(BigInt(payload.length))]);
  const bytes = new Uint8Array(prefix.length + payload.length);
  bytes.set(prefix);
  bytes.set(payload, prefix.length);
  return bytes;
}
function crc32(bytes: Uint8Array): number {
  // crc32fast uses IEEE CRC32, not CRC32C as the Rust client comment claims.
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++)
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
export function frame(payload: Uint8Array): Uint8Array<ArrayBuffer> {
  if (payload.length > MAX_FRAME_BYTES)
    throw new Error("Sync frame exceeds limit");
  const bytes = new Uint8Array(payload.length + 10);
  const view = new DataView(bytes.buffer);
  bytes[0] = 1;
  view.setUint32(2, payload.length, true);
  bytes.set(payload, 6);
  view.setUint32(6 + payload.length, crc32(payload), true);
  return bytes;
}
export function unframe(data: unknown, maxFrameBytes: number): Uint8Array {
  if (!(data instanceof ArrayBuffer) || data.byteLength < 10)
    throw new Error("Expected a complete binary sync frame");
  const view = new DataView(data);
  const length = view.getUint32(2, true);
  if (view.getUint8(0) !== 1 || view.getUint8(1) !== 0)
    throw new Error("Unsupported sync framing version or flags");
  if (length > maxFrameBytes) throw new Error("Sync frame exceeds limit");
  if (data.byteLength !== length + 10)
    throw new Error("Invalid sync frame length");
  const payload = new Uint8Array(data, 6, length);
  if (crc32(payload) !== view.getUint32(6 + length, true))
    throw new Error("Sync frame CRC mismatch");
  return payload;
}
class Reader {
  private offset = 0;
  private bytes: Uint8Array;
  constructor(bytes: Uint8Array) {
    this.bytes = bytes;
  }
  byte(): number {
    const value = this.bytes[this.offset++];
    if (value === undefined) throw new Error("Truncated sync message");
    return value;
  }
  uint(max = 0xffffffffffffffffn): bigint {
    let value = 0n;
    for (let shift = 0n; shift < 70n; shift += 7n) {
      const byte = this.byte();
      value |= BigInt(byte & 127) << shift;
      if (value > max) throw new Error("Sync integer exceeds limit");
      if (byte < 128) {
        if (shift > 0n && byte === 0)
          throw new Error("Non-canonical sync integer");
        return value;
      }
    }
    throw new Error("Invalid sync integer");
  }
  uuid(): string {
    if (this.uint(16n) !== 16n)
      throw new Error("Invalid replica or session UUID");
    const hex = Array.from({ length: 16 }, () =>
      this.byte().toString(16).padStart(2, "0"),
    ).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  }
  vector(): Uint8Array {
    const length = Number(this.uint(BigInt(this.bytes.length)));
    if (this.offset + length > this.bytes.length)
      throw new Error("Truncated sync vector");
    const value = this.bytes.slice(this.offset, this.offset + length);
    this.offset += length;
    return value;
  }
  done() {
    if (this.offset !== this.bytes.length)
      throw new Error("Trailing sync message bytes");
  }
}
export function response(payload: Uint8Array) {
  const reader = new Reader(payload);
  const tag = reader.byte();
  if (tag === 2) {
    const accepted = reader.byte();
    if (accepted > 1) throw new Error("Invalid attach acceptance");
    const backendReplicaId = reader.uuid();
    const sessionId = reader.uuid();
    const grantedDirection = directions[Number(reader.uint(2n))];
    if (!grantedDirection) throw new Error("Invalid granted direction");
    const maxFrameBytes = Number(reader.uint(0xffffffffn));
    const maxSessionBytes = reader.uint();
    reader.done();
    if (maxFrameBytes === 0 || maxSessionBytes === 0n)
      throw new Error("Invalid sync limits");
    return {
      type: "attach" as const,
      accepted: accepted === 1,
      backendReplicaId,
      sessionId,
      grantedDirection,
      maxFrameBytes,
      maxSessionBytes,
    };
  }
  if (tag === 3) {
    if (reader.uint(1n) !== 0n) throw new Error("Unsupported owner protocol");
    const data = reader.vector();
    reader.done();
    return { type: "owner" as const, data };
  }
  if (tag === 4) {
    const reason = Number(reader.uint(3n));
    if (reason === 3) reader.vector();
    reader.done();
    return { type: "abort" as const, reason };
  }
  if (tag === 5 || tag === 6) {
    reader.done();
    return { type: tag === 5 ? ("ping" as const) : ("pong" as const) };
  }
  throw new Error("Unexpected sync message");
}
