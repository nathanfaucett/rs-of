import { readFile } from "node:fs/promises";
import { Database, JsSqlSyncTransport, SqlHistoryRole } from "@ofsdk/sql";
import { indexedDB } from "fake-indexeddb";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { initSync } from "../../sql/pkg/ofdb_wasm.js";
import {
  frame,
  MAX_FRAME_BYTES,
  owner,
  response,
  unframe,
} from "../../storage-sync-client/src/protocol";
import fixtures from "../../storage-sync-client/src/wire-fixtures.json";
import { BrowserSyncSDK, createEstablishedSqlHandle, createSqlResourceSync } from "../src/browser";
import type { SyncConfig, SyncSDK } from "../src/core";
import { NodeSyncSDK } from "../src/node";

initSync({
  module: await readFile(new URL("../../sql/pkg/ofdb_wasm_bg.wasm", import.meta.url)),
});

const bytes = (hex: string) =>
  Uint8Array.from(hex.match(/../g) ?? [], (value) => Number.parseInt(value, 16));

class StalledSocket extends EventTarget {
  static latest: StalledSocket;
  static instances: StalledSocket[] = [];
  readonly url: string;
  binaryType = "blob";
  readyState = 0;
  closeCount = 0;
  sent: Uint8Array[] = [];

  constructor(url: string) {
    super();
    this.url = url;
    StalledSocket.latest = this;
    StalledSocket.instances.push(this);
    queueMicrotask(() => {
      this.readyState = 1;
      this.dispatchEvent(new Event("open"));
    });
  }

  send(data: Uint8Array) {
    this.sent.push(data);
    if (this.sent.length === 1) this.receiveBytes(data);
    if (this.sent.length === 2) this.receive(fixtures.acceptBidirectional);
  }

  receive(hex: string) {
    this.receiveBytes(bytes(hex));
  }

  receiveBytes(data: Uint8Array) {
    queueMicrotask(() => this.dispatchEvent(new MessageEvent("message", { data: data.buffer })));
  }

  close() {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this.closeCount++;
    this.dispatchEvent(new Event("close"));
  }
}

it("wraps an existing browser SQL database without opening storage or starting sync", async () => {
  vi.stubGlobal("indexedDB", indexedDB);
  const fetch = vi.fn();
  const socket = vi.fn();
  const tokenProvider = vi.fn(() => "storage-token");
  vi.stubGlobal("fetch", fetch);
  vi.stubGlobal("WebSocket", socket);
  const database = await Database.open("existing-local-database");
  const client = database.client();
  const open = vi.spyOn(indexedDB, "open");
  try {
    await client.executeRaw("CREATE TABLE items (id UUID PRIMARY KEY, value TEXT)");
    await client.executeRaw("INSERT INTO items (value) VALUES ('local data')");
    const identity = {
      storeId: "11111111-1111-1111-1111-111111111111",
      replicaId: "22222222-2222-2222-2222-222222222222",
      creationEpoch: 0n,
      schemaFingerprint: new Uint8Array(32),
    };
    const handle = createEstablishedSqlHandle(database, identity, {
      storageApiUrl: "https://node.test/storage",
      tokenProvider,
    });
    expect(handle.id).toBe(identity.storeId);
    await expect(handle.execute("SELECT value FROM items")).resolves.toEqual({
      columns: ["value"],
      rows: [["local data"]],
    });
    await handle.close();
    const localClient = database.client();
    try {
      await expect(localClient.executeRaw("SELECT value FROM items")).resolves.toMatchObject([
        { rows: [[{ type: "text", value: "local data" }]] },
      ]);
    } finally {
      localClient.close();
    }
    expect(open).not.toHaveBeenCalled();
    expect(fetch).not.toHaveBeenCalled();
    expect(tokenProvider).not.toHaveBeenCalled();
    expect(socket).not.toHaveBeenCalled();
  } finally {
    open.mockRestore();
    client.close();
    database.close();
    vi.unstubAllGlobals();
  }
});

it("keeps the original destination and token provider when caller inputs change", async () => {
  vi.stubGlobal("indexedDB", indexedDB);
  vi.stubGlobal(
    "WebSocket",
    class extends StalledSocket {
      override receive() {
        super.receive(fixtures.rejected);
      }
    }
  );
  const database = await Database.open("original-sync-destination");
  try {
    const identity = {
      storeId: "11111111-1111-1111-1111-111111111111",
      replicaId: "22222222-2222-2222-2222-222222222222",
      creationEpoch: 300n,
      schemaFingerprint: new Uint8Array(32).fill(0x33),
    };
    const options = {
      storageApiUrl: "https://node.test/storage",
      tokenProvider: () => "idp-token-é",
    };
    const handle = createEstablishedSqlHandle(database, identity, options);
    identity.storeId = "44444444-4444-4444-4444-444444444444";
    identity.replicaId = "55555555-5555-5555-5555-555555555555";
    identity.creationEpoch = 1n;
    identity.schemaFingerprint.fill(0xff);
    options.storageApiUrl = "https://other.test/storage";
    options.tokenProvider = () => "different-token";

    try {
      await expect(handle.sync()).rejects.toThrow("Sync attach rejected");
      const { sent, url } = StalledSocket.latest;
      expect(url).toBe("wss://node.test/storage/sync/ws");
      expect(sent).toEqual([bytes(fixtures.hello), bytes(fixtures.attachBidirectional)]);
      expect(handle.id).toBe("11111111-1111-1111-1111-111111111111");
    } finally {
      await handle.close();
    }
  } finally {
    database.close();
    vi.unstubAllGlobals();
  }
});

const config = (): SyncConfig => ({
  apiUrl: "https://node.test/storage",
  appId: "local-login",
  userId: "user",
  getToken: async () => "user-token",
  getStorageToken: async () => "storage-token",
});

async function createHandle(kind: "browser" | "node") {
  const fetch = vi.fn(async (input: unknown) => {
    const url = String(input);
    if (url.includes("/databases")) {
      return Response.json({
        id: "11111111-1111-1111-1111-111111111111",
        name: "lifecycle",
      });
    }
    return Response.json({ access_token: "storage-token" });
  });
  vi.stubGlobal("fetch", fetch);
  vi.stubGlobal("WebSocket", StalledSocket);
  const sdk: SyncSDK =
    kind === "browser" ? new BrowserSyncSDK(config()) : new NodeSyncSDK(config());
  const handle = await sdk.sql.create("lifecycle");
  return { sdk, handle };
}

async function flush() {
  for (let index = 0; index < 100; index++) {
    await vi.advanceTimersByTimeAsync(10);
    if (StalledSocket.latest.sent.length >= 3) return;
  }
  throw new Error("v7 exchange did not start");
}

describe.each(["browser", "node"] as const)("%s SQL sync lifecycle", (kind) => {
  beforeEach(() => {
    StalledSocket.instances = [];
    vi.stubGlobal("indexedDB", indexedDB);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("closes a stalled receive after the inactivity deadline", async () => {
    const { sdk, handle } = await createHandle(kind);
    vi.useFakeTimers();
    const states: string[] = [];
    handle.state$.subscribe((state) => states.push(state.status));
    const outcome = handle.sync().then(
      () => "unexpected success",
      (error: unknown) => String(error)
    );
    await flush();
    const socket = StalledSocket.latest;
    expect(socket.sent.length).toBeGreaterThanOrEqual(3);
    await vi.advanceTimersByTimeAsync(30_000);
    const result = await Promise.race([outcome, Promise.resolve("pending")]);
    socket.close();
    const settled = await outcome;
    await handle.close();
    await sdk.close();
    expect(result).not.toBe("pending");
    expect(settled).not.toBe("unexpected success");
    expect(socket.readyState).toBe(3);
    expect(states).toContain("error");
    expect(states).not.toContain("synced");
  });

  it("close aborts and awaits active synchronization", async () => {
    const { sdk, handle } = await createHandle(kind);
    vi.useFakeTimers();
    const states: string[] = [];
    handle.state$.subscribe((state) => states.push(state.status));
    const outcome = handle.sync().then(
      () => "unexpected success",
      (error: unknown) => String(error)
    );
    await flush();
    const socket = StalledSocket.latest;
    expect(socket.sent.length).toBeGreaterThanOrEqual(3);
    await handle.close();
    const readyAfterClose = socket.readyState;
    const closeCountAfterClose = socket.closeCount;
    const result = await Promise.race([outcome, Promise.resolve("pending")]);
    socket.close();
    const settled = await outcome;
    await sdk.close();
    expect(readyAfterClose).toBe(3);
    expect(closeCountAfterClose).toBe(1);
    expect(result).not.toBe("pending");
    expect(settled).not.toBe("unexpected success");
    expect(states).toContain("error");
    expect(states).not.toContain("synced");
  });

  it("does not attach after close during lifecycle initialization", async () => {
    const { sdk, handle } = await createHandle(kind);
    vi.useFakeTimers();
    const outcome = handle.sync().then(
      () => "unexpected success",
      (error: unknown) => String(error)
    );
    await handle.close();
    await vi.advanceTimersByTimeAsync(0);
    const sockets = [...StalledSocket.instances];
    const states = sockets.map((socket) => ({
      readyState: socket.readyState,
      closeCount: socket.closeCount,
    }));
    for (const socket of sockets) socket.close();
    const result = await outcome;
    await sdk.close();
    expect(sockets.length).toBeLessThanOrEqual(1);
    for (const state of states) {
      expect(state).toEqual({ readyState: 3, closeCount: 1 });
    }
    expect(result).not.toBe("unexpected success");
  });

  it("deduplicates concurrent sync initialization and closes the shared session", async () => {
    const { sdk, handle } = await createHandle(kind);
    vi.useFakeTimers();
    const first = handle.sync();
    const second = handle.sync();
    const outcomes = Promise.allSettled([first, second]);
    await flush();
    const sockets = [...StalledSocket.instances];
    for (const socket of sockets) socket.close();
    const results = await outcomes;
    await handle.close();
    await sdk.close();
    expect(sockets).toHaveLength(1);
    expect(sockets[0]?.closeCount).toBe(1);
    expect(results.map((result) => result.status)).toEqual(["rejected", "rejected"]);
  });
});

it.each(["published", "lost receipt", "destination has history"] as const)(
  "first upload on the already-open database: %s",
  async (result) => {
    vi.stubGlobal("indexedDB", indexedDB);
    const source = await Database.open(`sdk-first-upload-source-${result}`);
    const stage = await Database.open(`sdk-first-upload-stage-${result}`);
    const local = source.client();
    const staged = stage.client();
    const open = vi.spyOn(indexedDB, "open");
    let finishExchange!: () => void;
    let failExchange!: (reason: unknown) => void;
    const stagedExchange = new Promise<void>((resolve, reject) => {
      finishExchange = resolve;
      failExchange = reject;
    });
    let attempts = 0;
    vi.stubGlobal(
      "WebSocket",
      class extends StalledSocket {
        private readonly attempt = ++attempts;
        private readonly inbox = new TransformStream<Uint8Array, Uint8Array>();
        private readonly writer = this.inbox.writable.getWriter();

        override send(data: Uint8Array) {
          if (this.attempt > 1 && this.sent.length === 1) {
            this.sent.push(data);
            this.receive(fixtures.destinationHasHistory);
            return;
          }
          super.send(data);
          if (this.sent.length === 2) {
            const reader = this.inbox.readable.getReader();
            void stage
              .synchronizeHistory(
                new JsSqlSyncTransport({
                  send: async (payload: Uint8Array) => this.receiveBytes(frame(owner(payload))),
                  receive: async () => {
                    const { value } = await reader.read();
                    if (!value) throw new Error("Test socket closed");
                    return value;
                  },
                }),
                SqlHistoryRole.Responder
              )
              .then(finishExchange, failExchange);
          } else if (this.sent.length > 2) {
            const message = response(unframe(data.slice().buffer, MAX_FRAME_BYTES));
            if (message.type !== "owner") throw new Error("Expected SQL owner data");
            void this.writer.write(message.data).catch(() => {});
          }
        }

        override close() {
          super.close();
          void this.writer.abort().catch(() => {});
        }
      }
    );
    try {
      await local.executeRaw("CREATE TABLE items (id UUID PRIMARY KEY, value TEXT)");
      await local.executeRaw("INSERT INTO items (value) VALUES ('local data')");
      const identity = {
        storeId: "11111111-1111-1111-1111-111111111111",
        replicaId: "22222222-2222-2222-2222-222222222222",
        creationEpoch: 300n,
        schemaFingerprint: new Uint8Array(32).fill(0x33),
      };
      const options = {
        storageApiUrl: "https://node.test/storage",
        tokenProvider: () => "idp-token-é",
        mode: "first-upload" as const,
      };
      const sync = createSqlResourceSync(source, identity, options);
      identity.storeId = "44444444-4444-4444-4444-444444444444";
      identity.schemaFingerprint.fill(0xff);
      options.storageApiUrl = "https://other.test/storage";
      options.tokenProvider = () => "different-token";
      try {
        const pending = sync.sync();
        const onSuccess = vi.fn();
        void pending.then(onSuccess, () => {});
        await stagedExchange;
        const socket = StalledSocket.latest;
        expect(socket.url).toBe("wss://node.test/storage/sync/ws");
        expect(socket.sent.slice(0, 2)).toEqual([
          bytes(fixtures.hello),
          bytes(fixtures.firstUpload),
        ]);
        await expect(staged.executeRaw("SELECT value FROM items")).resolves.toMatchObject([
          { rows: [[{ type: "text", value: "local data" }]] },
        ]);
        expect(onSuccess).not.toHaveBeenCalled();
        if (result === "published") {
          socket.receive(fixtures.firstUploadPublished);
          await expect(pending).resolves.toBeUndefined();
        } else if (result === "lost receipt") {
          socket.close();
          await expect(pending).rejects.toMatchObject({ code: "transport" });
          await expect(sync.sync()).rejects.toMatchObject({ code: "destination-has-history" });
          expect(attempts).toBe(2);
          expect(StalledSocket.latest.url).toBe(socket.url);
          expect(StalledSocket.latest.sent).toEqual([
            bytes(fixtures.hello),
            bytes(fixtures.firstUpload),
          ]);
        } else {
          socket.receive(fixtures.destinationHasHistory);
          await expect(pending).rejects.toMatchObject({ code: "destination-has-history" });
          expect(attempts).toBe(1);
        }
      } finally {
        await sync.close();
      }
      await expect(local.executeRaw("SELECT value FROM items")).resolves.toMatchObject([
        { rows: [[{ type: "text", value: "local data" }]] },
      ]);
      expect(open).not.toHaveBeenCalled();
    } finally {
      open.mockRestore();
      local.close();
      staged.close();
      source.close();
      stage.close();
      vi.unstubAllGlobals();
    }
  }
);
