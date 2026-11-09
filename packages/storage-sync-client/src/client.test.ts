import { afterEach, expect, test, vi } from "vitest";
import { SyncClient } from "./index.js";
import fixtures from "./wire-fixtures.json";

const bytes = (hex: string) =>
  Uint8Array.from(hex.match(/../g) ?? [], (v) => Number.parseInt(v, 16));
const identity = {
  storeId: "11111111-1111-1111-1111-111111111111",
  replicaId: "22222222-2222-2222-2222-222222222222",
  creationEpoch: 300n,
  schemaFingerprint: new Uint8Array(32).fill(0x33),
};

class TestSocket extends EventTarget {
  static latest: TestSocket;
  static reply = fixtures.acceptBidirectional;
  static echo = true;
  static open = true;
  binaryType = "blob";
  sent: Uint8Array[] = [];
  readyState = 0;
  readonly url: string;
  constructor(url: string) {
    super();
    this.url = url;
    TestSocket.latest = this;
    queueMicrotask(() => {
      if (TestSocket.open) {
        this.readyState = 1;
        this.dispatchEvent(new Event("open"));
      }
    });
  }
  send(data: Uint8Array) {
    this.sent.push(data);
    if (this.sent.length === 1 && TestSocket.echo) this.receive(fixtures.hello);
    if (this.sent.length === 2 && TestSocket.reply)
      this.receive(TestSocket.reply);
  }
  receive(hex: string) {
    queueMicrotask(() =>
      this.dispatchEvent(
        new MessageEvent("message", { data: bytes(hex).buffer }),
      ),
    );
  }
  close() {
    this.readyState = 3;
    this.dispatchEvent(new Event("close"));
  }
}
afterEach(() => {
  vi.unstubAllGlobals();
  TestSocket.reply = fixtures.acceptBidirectional;
  TestSocket.echo = true;
  TestSocket.open = true;
});
const client = () => {
  vi.stubGlobal("WebSocket", TestSocket);
  return new SyncClient({
    storageApiUrl: "https://node.test/storage",
    tokenProvider: () => "idp-token-é",
  });
};

test("attaches with the storage server Postcard handshake, not the incompatible Rust client tags", async () => {
  vi.stubGlobal("WebSocket", TestSocket);
  const tokenProvider = vi.fn(() => "idp-token-é");
  const session = await new SyncClient({
    storageApiUrl: "https://node.test/storage/",
    tokenProvider,
  }).attach({ identity, direction: "Push" });
  expect(TestSocket.latest.url).toBe("wss://node.test/storage/sync/ws");
  expect(TestSocket.latest.binaryType).toBe("arraybuffer");
  expect(TestSocket.latest.sent).toEqual([
    bytes(fixtures.hello),
    bytes(fixtures.attachPush),
  ]);
  expect(tokenProvider).toHaveBeenCalledTimes(1);
  expect(session.result).toEqual({
    status: "attached",
    requestedDirection: "Push",
    grantedDirection: "Bidirectional",
    backendReplicaId: "44444444-4444-4444-4444-444444444444",
    sessionId: "55555555-5555-5555-5555-555555555555",
    maxFrameBytes: 16777216,
    maxSessionBytes: 1073741824n,
  });
  session.close();
});

test("keeps the attached socket usable for opaque ofdb SQL messages without claiming a backup", async () => {
  vi.stubGlobal("WebSocket", TestSocket);
  const session = await new SyncClient({
    storageApiUrl: "https://node.test/storage",
    tokenProvider: () => "idp-token-é",
  }).attach({ identity, direction: "Pull" });
  TestSocket.latest.receive(fixtures.owner);
  expect(await session.receive()).toEqual(Uint8Array.of(7, 8, 9));
  session.send(Uint8Array.of(7, 8, 9));
  expect(TestSocket.latest.sent[2]).toEqual(bytes(fixtures.owner));
  expect(session.result.status).toBe("attached");
  const pending = session.receive();
  session.close();
  await expect(pending).rejects.toMatchObject({ code: "cancelled" });
});

test.each(["Push", "Pull", "Bidirectional"] as const)(
  "requests and reports %s",
  async (direction) => {
    TestSocket.reply = fixtures[`accept${direction}`];
    const session = await client().attach({ identity, direction });
    expect(TestSocket.latest.sent[1]).toEqual(
      bytes(fixtures[`attach${direction}`]),
    );
    expect(session.result.requestedDirection).toBe(direction);
    expect(session.result.grantedDirection).toBe(direction);
    session.close();
  },
);

test.each([
  ["unauthorized", "auth"],
  ["rejected", "auth"],
  ["unavailable", "unavailable"],
  ["malformed", "protocol"],
  ["internal", "protocol"],
] as const)(
  "reports %s without exposing server details",
  async (reply, code) => {
    TestSocket.reply = fixtures[reply];
    await expect(
      client().attach({ identity, direction: "Push" }),
    ).rejects.toMatchObject({
      name: "SyncError",
      code,
      message: expect.not.stringContaining("private server details"),
    });
    expect(TestSocket.latest.readyState).toBe(3);
  },
);

test("rejects a grant that does not permit the requested direction", async () => {
  TestSocket.reply = fixtures.acceptPull;
  await expect(
    client().attach({ identity, direction: "Push" }),
  ).rejects.toMatchObject({
    code: "protocol",
    message: "Requested sync direction not granted",
  });
});

test.each(["", "   "])(
  "rejects an empty token before opening a socket",
  async (token) => {
    const factory = vi.fn();
    vi.stubGlobal("WebSocket", factory);
    await expect(
      new SyncClient({
        storageApiUrl: "https://node.test/storage",
        tokenProvider: () => token,
      }).attach({ identity, direction: "Push" }),
    ).rejects.toMatchObject({ code: "auth" });
    expect(factory).not.toHaveBeenCalled();
  },
);

test("does not expose token provider errors", async () => {
  await expect(
    new SyncClient({
      storageApiUrl: "https://node.test/storage",
      tokenProvider: () => {
        throw new Error("secret-token");
      },
    }).attach({ identity, direction: "Push" }),
  ).rejects.toMatchObject({
    code: "auth",
    message: "Could not obtain sync access token",
  });
});

test("cancels while the token provider is pending and never opens a socket later", async () => {
  const controller = new AbortController();
  const factory = vi.fn();
  vi.stubGlobal("WebSocket", factory);
  let supply: (token: string) => void = () => {};
  const pending = new SyncClient({
    storageApiUrl: "https://node.test",
    tokenProvider: () =>
      new Promise<string>((resolve) => {
        supply = resolve;
      }),
  }).attach({ identity, direction: "Push", signal: controller.signal });
  controller.abort();
  await expect(pending).rejects.toMatchObject({ code: "cancelled" });
  supply("secret-token");
  await Promise.resolve();
  expect(factory).not.toHaveBeenCalled();
});

test("cancels a stalled connection", async () => {
  TestSocket.open = false;
  const controller = new AbortController();
  const pending = client().attach({
    identity,
    direction: "Push",
    signal: controller.signal,
  });
  await Promise.resolve();
  controller.abort();
  await expect(pending).rejects.toMatchObject({ code: "cancelled" });
  expect(TestSocket.latest.readyState).toBe(3);
});

test("cancels an attached session and rejects pending and future receives", async () => {
  const controller = new AbortController();
  const session = await client().attach({
    identity,
    direction: "Push",
    signal: controller.signal,
  });
  const pending = session.receive();
  controller.abort();
  await expect(pending).rejects.toMatchObject({ code: "cancelled" });
  await expect(session.receive()).rejects.toMatchObject({
    code: "cancelled",
  });
  expect(() => session.send(Uint8Array.of(1))).toThrow();
});

test.each(["close", "error"])(
  "rejects a transport %s during attach",
  async (event) => {
    TestSocket.reply = "";
    const pending = client().attach({ identity, direction: "Push" });
    await Promise.resolve();
    TestSocket.latest.dispatchEvent(new Event(event));
    await expect(pending).rejects.toMatchObject({ code: "transport" });
  },
);

test.each(["close", "error"])(
  "rejects a pending receive on transport %s",
  async (event) => {
    const session = await client().attach({ identity, direction: "Push" });
    const pending = session.receive();
    TestSocket.latest.dispatchEvent(new Event(event));
    await expect(pending).rejects.toMatchObject({ code: "transport" });
  },
);

test.each([
  "http://user:pass@node.test/storage",
  "https://node.test/storage?token=secret",
  "https://node.test/storage#secret",
  "file:///storage",
  "wss://node.test/storage",
])("rejects unsafe or unsupported API URL %s", (storageApiUrl) => {
  expect(
    () => new SyncClient({ storageApiUrl, tokenProvider: () => "token" }),
  ).toThrow();
});

test.each([
  ["http://localhost:8080/storage", "ws://localhost:8080/storage/sync/ws"],
  [
    "https://node.test/custom/storage///",
    "wss://node.test/custom/storage/sync/ws",
  ],
  ["https://node.test", "wss://node.test/sync/ws"],
])(
  "preserves the configured API prefix %s",
  async (storageApiUrl, expected) => {
    vi.stubGlobal("WebSocket", TestSocket);
    const session = await new SyncClient({
      storageApiUrl,
      tokenProvider: () => "token",
    }).attach({ identity, direction: "Push" });
    expect(TestSocket.latest.url).toBe(expected);
    session.close();
  },
);

test.each([
  { ...identity, storeId: "not-a-uuid" },
  { ...identity, creationEpoch: -1n },
  { ...identity, creationEpoch: 18446744073709551616n },
  { ...identity, schemaFingerprint: new Uint8Array(31) },
])(
  "rejects an invalid store identity without opening a socket",
  async (invalid) => {
    const factory = vi.fn();
    vi.stubGlobal("WebSocket", factory);
    await expect(
      new SyncClient({
        storageApiUrl: "https://node.test",
        tokenProvider: () => "token",
      }).attach({ identity: invalid, direction: "Push" }),
    ).rejects.toMatchObject({ code: "protocol" });
    expect(factory).not.toHaveBeenCalled();
  },
);

test.each([
  "version",
  "flags",
  "crc",
  "truncated",
  "trailing",
  "oversized",
  "text",
])("rejects %s frames and closes the socket", async (fault) => {
  TestSocket.reply = "";
  const pending = client().attach({ identity, direction: "Push" });
  await Promise.resolve();
  await Promise.resolve();
  let data: ArrayBuffer | string = bytes(fixtures.acceptPush).buffer!;
  const view = new Uint8Array(data);
  if (fault === "version") view[0] = 2;
  if (fault === "flags") view[1] = 2;
  if (fault === "crc" && view.length > 0) view[view.length - 1]! ^= 1;
  if (fault === "truncated") data = data.slice(0, -1);
  if (fault === "trailing") data = new Uint8Array([...view, 0]).buffer;
  if (fault === "oversized") new DataView(data).setUint32(2, 16777217, true);
  if (fault === "text") data = "not binary";
  TestSocket.latest.dispatchEvent(new MessageEvent("message", { data }));
  await expect(pending).rejects.toMatchObject({ code: "protocol" });
  expect(TestSocket.latest.readyState).toBe(3);
});

test("answers the server Ping using its actual Pong tag", async () => {
  TestSocket.reply = fixtures.ping;
  const pending = client().attach({ identity, direction: "Push" });
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  expect(TestSocket.latest.sent[2]).toEqual(bytes(fixtures.pong));
  TestSocket.latest.receive(fixtures.acceptPush);
  (await pending).close();
});

test("rejects negotiated session limits already exceeded by the handshake", async () => {
  TestSocket.reply = fixtures.tinySession;
  await expect(
    client().attach({ identity, direction: "Push" }),
  ).rejects.toMatchObject({ code: "protocol" });
  expect(TestSocket.latest.readyState).toBe(3);
});

test("counts the envelope in the negotiated frame limit", async () => {
  TestSocket.reply = fixtures.tinyFrame;
  const session = await client().attach({ identity, direction: "Push" });
  expect(() => session.send(new Uint8Array(6))).toThrow(
    "Sync frame exceeds negotiated limit",
  );
  expect(TestSocket.latest.readyState).toBe(3);
  await expect(session.receive()).rejects.toMatchObject({ code: "protocol" });
});

test("enforces the cumulative negotiated session budget", async () => {
  TestSocket.reply = fixtures.budget;
  const session = await client().attach({ identity, direction: "Push" });
  session.send(new Uint8Array(100));
  expect(() => session.send(new Uint8Array(200))).toThrow(
    "Sync session exceeds byte limit",
  );
  expect(TestSocket.latest.readyState).toBe(3);
});

test("rejects inbound owner messages above the negotiated frame limit", async () => {
  TestSocket.reply = fixtures.tinyFrame;
  const session = await client().attach({ identity, direction: "Push" });
  const pending = session.receive();
  TestSocket.latest.dispatchEvent(
    new MessageEvent("message", {
      data: bytes(fixtures.largeOwner).buffer,
    }),
  );
  await expect(pending).rejects.toMatchObject({ code: "protocol" });
});

test("retains owner data received before the consumer starts reading", async () => {
  const session = await client().attach({ identity, direction: "Push" });
  TestSocket.latest.receive(fixtures.owner);
  TestSocket.latest.receive(fixtures.owner);
  await Promise.resolve();
  expect(await session.receive()).toEqual(Uint8Array.of(7, 8, 9));
  expect(await session.receive()).toEqual(Uint8Array.of(7, 8, 9));
  session.close();
});

test("reports authorization loss after attach", async () => {
  const session = await client().attach({ identity, direction: "Push" });
  const pending = session.receive();
  TestSocket.latest.receive(fixtures.unauthorized);
  await expect(pending).rejects.toMatchObject({ code: "auth" });
  expect(TestSocket.latest.readyState).toBe(3);
});

test("rejects oversized outgoing owner payloads as protocol errors and closes the session", async () => {
  const session = await client().attach({ identity, direction: "Push" });
  expect(() => session.send(new Uint8Array(16777216))).toThrowError(
    expect.objectContaining({ code: "protocol" }),
  );
  expect(TestSocket.latest.readyState).toBe(3);
});

test.each([
  "invalidBoolean",
  "trailingPostcard",
  "wrongClientAbortTag",
] as const)(
  "rejects malformed Postcard %s with a valid frame checksum",
  async (reply) => {
    TestSocket.reply = fixtures[reply];
    await expect(
      client().attach({ identity, direction: "Push" }),
    ).rejects.toMatchObject({ code: "protocol" });
    expect(TestSocket.latest.readyState).toBe(3);
  },
);

test("preserves all u64 creation epoch bits in server-compatible messages", async () => {
  TestSocket.echo = false;
  const session = await client().attach({
    identity: { ...identity, creationEpoch: 18446744073709551615n },
    direction: "Push",
  });
  expect(TestSocket.latest.sent).toEqual([
    bytes(fixtures.maxEpochHello),
    bytes(fixtures.maxEpochAttach),
  ]);
  session.close();
});

test("decodes full-width server limits without rounding and caps them to local protocol limits", async () => {
  TestSocket.reply = fixtures.maxLimits;
  const session = await client().attach({ identity, direction: "Push" });
  expect(session.result.maxFrameBytes).toBe(16777216);
  expect(session.result.maxSessionBytes).toBe(1073741824n);
  session.close();
});

test("obtains a fresh token for every attach", async () => {
  vi.stubGlobal("WebSocket", TestSocket);
  const tokenProvider = vi.fn(() => "idp-token-é");
  const shared = new SyncClient({
    storageApiUrl: "https://node.test/storage",
    tokenProvider,
  });
  (await shared.attach({ identity, direction: "Push" })).close();
  (await shared.attach({ identity, direction: "Pull" })).close();
  expect(tokenProvider).toHaveBeenCalledTimes(2);
});

test("delivers complete buffered owner messages before reporting remote close", async () => {
  const session = await client().attach({ identity, direction: "Pull" });
  TestSocket.latest.receive(fixtures.owner);
  await Promise.resolve();
  TestSocket.latest.dispatchEvent(new Event("close"));
  expect(await session.receive()).toEqual(Uint8Array.of(7, 8, 9));
  await expect(session.receive()).rejects.toMatchObject({
    code: "transport",
  });
});

test("preserves a large valid token in the binary attach request", async () => {
  vi.stubGlobal("WebSocket", TestSocket);
  const token = "a".repeat(131072);
  const session = await new SyncClient({
    storageApiUrl: "https://node.test/storage",
    tokenProvider: () => token,
  }).attach({ identity, direction: "Push" });
  const request = TestSocket.latest.sent[1];
  expect(request?.slice(75, 78)).toEqual(Uint8Array.of(128, 128, 8));
  expect(request?.slice(78, -5)).toEqual(new TextEncoder().encode(token));
  session.close();
});

test("reports malformed API URLs as a protocol error", () => {
  expect(
    () =>
      new SyncClient({
        storageApiUrl: "not a URL",
        tokenProvider: () => "token",
      }),
  ).toThrowError(
    expect.objectContaining({ name: "SyncError", code: "protocol" }),
  );
});
