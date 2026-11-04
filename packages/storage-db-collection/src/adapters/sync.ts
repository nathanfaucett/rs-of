import type { ConnectBackend, StorageBackend } from "../adapters/backend.js";

export interface SyncWebSocket {
  send(data: Uint8Array): void;
  close(): void;
  addEventListener(
    type: string,
    listener: (event: MessageEvent) => void,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: (event: MessageEvent) => void,
    options?: boolean | EventListenerOptions,
  ): void;
  readyState: number;
}

export interface SyncClient {
  connect(
    url: string,
    tokenProvider: () => Promise<string>,
    storeIdentity: StoreIdentity,
    replicaId: string,
  ): Promise<SyncClientInstance>;
  disconnect(): void;
}

export interface SyncClientInstance {
  sync(
    direction: SyncDirection,
    database: any,
    namespace: any,
    stagingDir: string,
  ): Promise<SyncResult>;
}

export interface StoreIdentity {
  store_id: string;
  replica_id: string;
  creation_epoch: number;
  schema_fingerprint: Uint8Array;
}

export type SyncDirection = "push" | "pull" | "bidirectional";

export interface SyncResult {
  receivedUnits: number;
  sentUnits: number;
  receivedChanges: number;
  sentChanges: number;
  receivedSnapshots: number;
  sentSnapshots: number;
  acknowledged_bytes: number;
}

/**
 * Creates a sync backend that uses the sync WebSocket protocol.
 * This bridges TanStack DB's sync protocol to the sync WebSocket.
 */
export function createSyncBackend(
  connect: (
    url: string,
    tokenProvider: () => Promise<string>,
    storeIdentity: StoreIdentity,
    replicaId: string,
  ) => Promise<SyncClientInstance>,
  url: string,
  tokenProvider: () => Promise<string>,
  storeIdentity: StoreIdentity,
  replicaId: string,
): ConnectBackend {
  let clientInstance: SyncClientInstance | null = null;

  return async (signal?: AbortSignal) => {
    if (!clientInstance) {
      clientInstance = await connect(
        url,
        tokenProvider,
        storeIdentity,
        replicaId,
      );
    }

    const backend: StorageBackend = {
      async read(key: string): Promise<Uint8Array | undefined> {
        // For sync backend, reads come from local database after sync
        // This is a placeholder - actual implementation would use the synced local DB
        throw new Error(
          "Sync backend read not implemented - use local DB after sync",
        );
      },

      async write(key: string, bytes: Uint8Array): Promise<void> {
        // For sync backend, writes go to local database then sync
        // This is a placeholder - actual implementation would use the synced local DB
        throw new Error(
          "Sync backend write not implemented - use local DB after sync",
        );
      },

      async delete(key: string): Promise<void> {
        throw new Error(
          "Sync backend delete not implemented - use local DB after sync",
        );
      },

      async list(prefix: string): Promise<string[]> {
        throw new Error(
          "Sync backend list not implemented - use local DB after sync",
        );
      },

      close(): void {
        clientInstance = null;
      },

      onDisconnect(handler: () => void): void {
        // The sync client handles disconnection internally
      },
    };

    return backend;
  };
}

/**
 * Type definitions for the sync protocol (matching Rust protocol)
 */
export enum ProtocolId {
  OfdbSqlV7 = "ofdb-sql-v7",
  OfdbKvV1 = "ofdb-kv-v1",
  OffsFsV1 = "offs-fs-v1",
}

export enum AttachDirection {
  Push = 0,
  Pull = 1,
  Bidirectional = 2,
}

export interface Hello {
  protocol_version: number;
  manifest: Uint8Array;
  store_identity: StoreIdentity;
  supported_protocols: ProtocolId[];
}

export interface AttachRequest {
  store_identity: StoreIdentity;
  auth_token: Uint8Array;
  direction: AttachDirection;
  reconnect_token?: ReconnectToken;
}

export interface AttachResponse {
  accepted: boolean;
  backend_replica_id: string;
  session_id: string;
  granted_direction: AttachDirection;
  max_frame_bytes: number;
  max_session_bytes: number;
}

export interface ReconnectToken {
  session_id: string;
  last_acknowledged_bytes: number;
  last_acknowledged_units: SyncUnitId[];
  sender_state_hash: Uint8Array;
  receiver_state_hash: Uint8Array;
  timestamp_ms: number;
}

export interface SyncUnitId {
  protocol_id: ProtocolId;
  unit_key: string;
}

export type SyncMessage =
  | { type: "hello"; hello: Hello }
  | { type: "attachRequest"; attach_request: AttachRequest }
  | { type: "attachResponse"; attach_response: AttachResponse }
  | { type: "ownerMessage"; protocol_id: ProtocolId; payload: Uint8Array }
  | {
      type: "ownerMessageBatch";
      protocol_id: ProtocolId;
      payloads: Uint8Array[];
    }
  | {
      type: "ack";
      acknowledged_bytes: number;
      acknowledged_units: SyncUnitId[];
      receiver_state_hash: Uint8Array;
    }
  | {
      type: "progress";
      phase: number;
      bytes_sent: number;
      bytes_received: number;
      units_completed: number;
      units_total: number;
      eta_ms?: number;
    }
  | { type: "cancel"; reason: string }
  | { type: "reconnect"; reconnect_token: ReconnectToken }
  | { type: "abort"; reason: string }
  | { type: "ping" }
  | { type: "pong" };

const FRAMING_VERSION = 1;
const MAX_FRAME_PAYLOAD = 16 * 1024 * 1024;

function crc32c(data: Uint8Array): number {
  // Simple CRC32C implementation
  let crc = 0xffffffff;
  for (const byte of data) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) {
      crc = (crc >>> 1) ^ (0x82f63b78 & -(crc & 1));
    }
  }
  return ~crc >>> 0;
}

function encodeFrame(
  flags: number,
  payload: Uint8Array,
  continuation: boolean,
): Uint8Array {
  const frameFlags = continuation ? flags | 0x01 : flags;
  const length = payload.length;

  const header = new Uint8Array(6);
  header[0] = FRAMING_VERSION;
  header[1] = frameFlags;
  header[2] = length & 0xff;
  header[3] = (length >>> 8) & 0xff;
  header[4] = (length >>> 16) & 0xff;
  header[5] = (length >>> 24) & 0xff;

  const crc = crc32c(payload);
  const crcBytes = new Uint8Array(4);
  crcBytes[0]! = crc & 0xff;
  crcBytes[1]! = (crc >>> 8) & 0xff;
  crcBytes[2]! = (crc >>> 16) & 0xff;
  crcBytes[3]! = (crc >>> 24) & 0xff;

  const frame = new Uint8Array(6 + length + 4);
  frame.set(header, 0);
  frame.set(payload, 6);
  frame.set(crcBytes, 6 + length);

  return frame;
}

function decodeFrame(
  buffer: Uint8Array,
): { payload: Uint8Array; remaining: Uint8Array } | null {
  if (buffer.length < 10) return null; // Minimum frame size

  const version = buffer[0];
  if (version !== FRAMING_VERSION) {
    throw new Error(`Invalid frame version: ${version}`);
  }

  const flags = buffer[1];
  const length =
    buffer[2]! | (buffer[3]! << 8) | (buffer[4]! << 16) | (buffer[5]! << 24);

  if (length > MAX_FRAME_PAYLOAD) {
    throw new Error(`Frame too large: ${length}`);
  }

  const totalNeeded = 6 + length + 4;
  if (buffer.length < totalNeeded) return null;

  const payload = buffer.slice(6, 6 + length);
  const crcBytes = buffer.slice(6 + length, 6 + length + 4);
  const expectedCrc =
    crcBytes[0]! |
    (crcBytes[1]! << 8) |
    (crcBytes[2]! << 16) |
    (crcBytes[3]! << 24);

  const actualCrc = crc32c(payload);
  if (actualCrc !== expectedCrc) {
    throw new Error(`CRC mismatch: expected ${expectedCrc}, got ${actualCrc}`);
  }

  const remaining = buffer.slice(totalNeeded);
  return { payload, remaining };
}

// Postcard encoding/decoding (simplified - in practice use a proper postcard WASM build)
function postcardEncode(value: any): Uint8Array {
  // This is a placeholder - real implementation would use postcard WASM
  return new TextEncoder().encode(JSON.stringify(value));
}

function postcardDecode<T>(bytes: Uint8Array): T {
  // This is a placeholder - real implementation would use postcard WASM
  return JSON.parse(new TextDecoder().decode(bytes));
}

/**
 * Native sync client implementation using WebSocket
 */
export class NativeSyncClient {
  private ws: WebSocket | null = null;
  private url: string;
  private tokenProvider: () => Promise<string>;
  private store_identity: StoreIdentity;
  private replica_id: string;
  private frameBuffer: Uint8Array = new Uint8Array();
  private pendingRequests: Map<
    number,
    { resolve: (value: SyncMessage) => void; reject: (error: Error) => void }
  > = new Map();
  private requestId = 0;
  private connected = false;

  constructor(
    url: string,
    tokenProvider: () => Promise<string>,
    storeIdentity: StoreIdentity,
    replicaId: string,
  ) {
    this.url = url;
    this.tokenProvider = tokenProvider;
    this.store_identity = storeIdentity;
    this.replica_id = replicaId;
  }

  async connect(): Promise<void> {
    const token = await this.tokenProvider();
    const wsUrl = new URL(this.url);
    wsUrl.protocol = wsUrl.protocol === "https:" ? "wss:" : "ws:";
    wsUrl.searchParams.set("token", token);

    this.ws = new WebSocket(wsUrl.toString());
    this.ws.binaryType = "arraybuffer";

    return new Promise((resolve, reject) => {
      if (!this.ws) {
        reject(new Error("WebSocket not created"));
        return;
      }

      this.ws.onopen = () => {
        this.connected = true;
        this.sendHello();
        resolve();
      };

      this.ws.onerror = (event) => {
        reject(new Error("WebSocket connection failed"));
      };

      this.ws.onmessage = (event) => {
        if (event.data instanceof ArrayBuffer) {
          const data = new Uint8Array(event.data);
          this.frameBuffer = concatArrays(this.frameBuffer, data);
          this.processFrames();
        }
      };

      this.ws.onclose = () => {
        this.connected = false;
      };
    });
  }

  private sendHello() {
    const hello: Hello = {
      protocol_version: 1,
      manifest: new Uint8Array(),
      store_identity: this.store_identity,
      supported_protocols: [ProtocolId.OfdbSqlV7],
    };
    this.sendMessage({ type: "hello", hello });
  }

  private sendMessage(message: SyncMessage) {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;

    const encoded = postcardEncode(message);
    const frame = encodeFrame(0, encoded, false);
    this.ws.send(frame.buffer);
  }

  private processFrames() {
    while (true) {
      const result = decodeFrame(this.frameBuffer);
      if (!result) break;

      this.frameBuffer = result.remaining;
      const message = postcardDecode<SyncMessage>(result.payload);
      this.handleMessage(message);
    }
  }

  private handleMessage(message: SyncMessage) {
    // Handle incoming sync messages
    // This would be expanded to handle the full sync protocol
    console.log("Received sync message:", message);
  }

  async sync(
    direction: SyncDirection,
    database: any,
    namespace: any,
    stagingDir: string,
  ): Promise<SyncResult> {
    // This is a placeholder - actual implementation would run the sync protocol
    throw new Error("Sync not fully implemented in TypeScript yet");
  }

  disconnect() {
    this.ws?.close();
    this.ws = null;
    this.connected = false;
  }
}

function concatArrays(a: Uint8Array, b: Uint8Array): Uint8Array {
  const result = new Uint8Array(a.length + b.length);
  result.set(a, 0);
  result.set(b, a.length);
  return result;
}

/**
 * Convenience function to create a native sync client
 */
export async function createNativeSyncClient(
  url: string,
  tokenProvider: () => Promise<string>,
  storeIdentity: StoreIdentity,
  replicaId: string,
): Promise<NativeSyncClient> {
  const client = new NativeSyncClient(
    url,
    tokenProvider,
    storeIdentity,
    replicaId,
  );
  await client.connect();
  return client;
}
