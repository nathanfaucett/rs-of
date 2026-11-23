import { readFile } from "node:fs/promises";
import { indexedDB } from "fake-indexeddb";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { initSync } from "../../sql/pkg/ofdb_wasm.js";
import fixtures from "../../storage-sync-client/src/wire-fixtures.json";
import { BrowserSyncSDK } from "../src/browser";
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
