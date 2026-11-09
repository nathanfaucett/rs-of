import { createLiveQueryCollection } from "@tanstack/db";
import { afterEach, expect, it, vi } from "vitest";
import * as v from "valibot";
import { createStorageCollection } from "./index.js";

const schema = v.object({
  id: v.string(),
  name: v.string(),
  favorite: v.boolean(),
});
const record = { id: "one", name: "Alpha", favorite: false };
const entry = (name: string) => ({ name, hash: "hash", size: 1, local: true });
const content = (value: unknown): { type: "read"; content: number[] } => ({
  type: "read",
  content: [...new TextEncoder().encode(JSON.stringify(value))],
});

const collections: { cleanup(): Promise<void> }[] = [];
afterEach(async () => {
  for (const collection of collections.splice(0).reverse())
    await collection.cleanup();
});

interface MockBackend {
  read: (key: string) => Promise<Uint8Array | undefined>;
  write: (key: string, bytes: Uint8Array) => Promise<void>;
  delete: (key: string) => Promise<void>;
  list: (prefix: string) => Promise<string[]>;
  close?: () => Promise<void> | void;
  onDisconnect?: (handler: () => void) => void;
  waitForWrite?: () => Promise<unknown>;
}

function createMockBackend(
  respond: (request: {
    type: string;
    path: string;
    content?: number[];
  }) => Promise<unknown>,
): MockBackend {
  let disconnectHandler: (() => void) | undefined;
  const writePromises: Array<{ resolve: (value: unknown) => void }> = [];

  return {
    async read(key: string) {
      const response = (await respond({ type: "read", path: key })) as {
        type: "read";
        content: number[];
      };
      if (response.type === "read" && response.content) {
        return new Uint8Array(response.content);
      }
      throw new Error(`Storage record read failed: ${response.type}`);
    },
    async write(key: string, bytes: Uint8Array) {
      const response = (await respond({
        type: "write",
        path: key,
        content: [...bytes],
      })) as { type: "written"; entry: { name: string } };
      for (const p of writePromises) p.resolve(response);
      writePromises.length = 0;
      if (response.type !== "written" || response.entry.name !== key) {
        throw new Error(`Write not acknowledged for ${key}: ${response.type}`);
      }
    },
    async delete(key: string) {
      const response = (await respond({ type: "delete", path: key })) as {
        type: "deleted";
      };
      if (response.type !== "deleted") {
        throw new Error(`Delete not acknowledged for ${key}`);
      }
    },
    async list(prefix: string) {
      const response = (await respond({ type: "list", path: prefix })) as {
        type: "listed";
        entries: { name: string }[];
      };
      if (response.type !== "listed") {
        throw new Error(`List failed for ${prefix}: ${response.type}`);
      }
      return response.entries.map((e) => e.name);
    },
    onDisconnect(handler: () => void) {
      disconnectHandler = handler;
    },
    close() {
      disconnectHandler?.();
    },
    waitForWrite() {
      return new Promise((resolve) => writePromises.push({ resolve }));
    },
  };
}

function setup(
  respond?: (request: {
    type: string;
    path: string;
    content?: number[];
  }) => Promise<unknown>,
  requestTimeoutMs?: number,
) {
  const backend = createMockBackend(
    respond ?? (async () => ({ type: "listed", entries: [] })),
  );
  const connect = vi.fn(async () => backend);
  const collection = createStorageCollection({
    connect,
    userId: "user",
    applicationId: "app",
    directory: "secrets",
    schema,
    getKey: (value) => value.id,
    requestTimeoutMs,
  });
  collections.push(collection);
  return { collection, connect, backend };
}

it("loads validated records through Storage into a real ordered live query", async () => {
  const { collection, connect } = setup(async (request) =>
    request.type === "list"
      ? {
          type: "listed",
          entries: [
            entry("secrets/one.json"),
            entry("secrets/two.json"),
            entry("secrets/ignore.txt"),
          ],
        }
      : request.type === "read"
        ? content(
            request.path === "secrets/one.json"
              ? record
              : { id: "two", name: "Zulu", favorite: true },
          )
        : { type: "error", code: "operationFailed" },
  );
  const query = createLiveQueryCollection({
    query: (q) =>
      q
        .from({ secret: collection })
        .orderBy(({ secret }) => secret.favorite, "desc")
        .orderBy(({ secret }) => secret.name, "asc"),
  });
  collections.push(query);
  await query.preload();
  expect([...query.values()].map((value) => value.id)).toEqual(["two", "one"]);
  expect(collection.status).toBe("ready");
  expect(connect).toHaveBeenCalledTimes(1);
});

it("marks an empty directory ready without mistaking a failed listing for empty", async () => {
  const { collection } = setup();
  expect(collection.status).toBe("idle");
  await collection.preload();
  expect(collection.status).toBe("ready");
  expect(collection.size).toBe(0);
});

it.each(["../escape", "nested/escape", "%2e%2e", "", "a\\b"])(
  "rejects unsafe record key %s before reading it",
  async (key) => {
    const { collection } = setup(async (request) =>
      request.type === "list"
        ? { type: "listed", entries: [entry(`secrets/${key}.json`)] }
        : { type: "error", code: "operationFailed" },
    );
    await expect(collection.preload()).rejects.toThrow("Unsafe record key");
    expect(collection.status).toBe("error");
    expect(collection.size).toBe(0);
  },
);

it.each([
  { type: "error", code: "operationFailed" },
  { type: "read", content: [123] },
  content({ ...record, favorite: "invalid" }),
  content({ ...record, id: "other" }),
])("never publishes a partial or malformed snapshot (%j)", async (response) => {
  const { collection } = setup(async (request) =>
    request.type === "list"
      ? {
          type: "listed",
          entries: [entry("secrets/one.json"), entry("secrets/two.json")],
        }
      : request.path.endsWith("one.json")
        ? content(record)
        : response,
  );
  await expect(collection.preload()).rejects.toThrow();
  expect(collection.status).toBe("error");
  expect(collection.size).toBe(0);
});

it("disposes during a held listing without reading or publishing late records", async () => {
  const held = Promise.withResolvers<{ type: string; entries?: unknown[] }>();
  const { collection, backend } = setup(() => held.promise);
  const loading = collection.preload().catch(() => undefined);
  await vi.waitFor(() => expect(backend).toBeDefined());
  await collection.cleanup();
  held.resolve({ type: "listed", entries: [entry("secrets/one.json")] });
  await loading;
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(collection.size).toBe(0);
  await expect(collection.preload()).rejects.toThrow("disposed");
});

it("does not initiate work if disposed before loading", async () => {
  const { collection } = setup();
  await collection.cleanup();
  await expect(collection.preload()).rejects.toThrow("disposed");
});

it("does not publish a validated record if the connection closes during validation", async () => {
  const { backend } = setup(async (request) =>
    request.type === "list"
      ? { type: "listed", entries: [entry("secrets/one.json")] }
      : content(record),
  );
  const validation = Promise.withResolvers<{ value: typeof record }>();
  const validating = vi.fn(() => validation.promise);
  const collection = createStorageCollection({
    connect: vi.fn(async () => backend),
    userId: "user",
    applicationId: "app",
    directory: "secrets",
    getKey: (value: typeof record) => value.id,
    schema: {
      "~standard": { version: 1, vendor: "test", validate: validating },
    },
  });
  collections.push(collection);
  const loading = collection.preload().catch(() => undefined);
  await vi.waitFor(() => expect(validating).toHaveBeenCalledOnce());
  backend.close?.();
  validation.resolve({ value: record });
  await loading;
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(collection.status).toBe("error");
  expect(collection.size).toBe(0);
});

it.each([
  "../secrets",
  "/secrets",
  "secrets/",
  "secrets//nested",
  "secrets/%2e%2e",
])("rejects unsafe directory %s before transport", (directory) => {
  expect(() =>
    createStorageCollection({
      connect: async () => ({
        read: async () => undefined,
        write: async () => {},
        delete: async () => {},
        list: async () => [],
      }),
      userId: "user",
      applicationId: "app",
      directory,
      schema,
      getKey: (value) => value.id,
    }),
  ).toThrow("Unsafe record directory");
});

it("optimistically inserts but settles only after authoritative reconciliation, then reopens persisted CRUD", async () => {
  const files = new Map<string, unknown>();
  const held = Promise.withResolvers<
    { type: "read"; content: number[] } | { type: "error"; code: string }
  >();
  let hold = true;
  const respond = async (request: {
    type: string;
    path: string;
    content?: number[];
  }): Promise<
    | { type: "written"; entry: { name: string } }
    | { type: "deleted" }
    | { type: "listed"; entries: { name: string }[] }
    | { type: "read"; content: number[] }
    | { type: "error"; code: string }
  > => {
    if (request.type === "write") {
      files.set(
        request.path,
        JSON.parse(
          new TextDecoder().decode(new Uint8Array(request.content || [])),
        ),
      );
      return { type: "written", entry: entry(request.path) };
    }
    if (request.type === "delete") {
      files.delete(request.path);
      return { type: "deleted" };
    }
    if (request.type === "list")
      return { type: "listed", entries: [...files.keys()].map(entry) };
    if (request.type === "read") {
      if (hold) return held.promise;
      return content(files.get(request.path));
    }
    return { type: "error", code: "operationFailed" };
  };
  const { collection, backend } = setup(respond);
  await collection.preload();
  const tx = collection.insert(record);
  expect(collection.get("one")).toMatchObject(record);
  await backend.waitForWrite?.();
  hold = false;
  const normalized = { ...record, name: "Server Alpha" };
  files.set("secrets/one.json", normalized);
  held.resolve(content(normalized));
  await tx.isPersisted.promise;
  expect(collection.get("one")).toMatchObject(normalized);
  await collection.update("one", (draft) => {
    draft.name = "Edited";
  }).isPersisted.promise;
  await collection.cleanup();
  const reopened = setup(respond).collection;
  await reopened.preload();
  expect(reopened.get("one")?.name).toBe("Edited");
  await reopened.delete("one").isPersisted.promise;
  expect(reopened.size).toBe(0);
  const final = setup(respond).collection;
  await final.preload();
  expect(final.size).toBe(0);
});

it("does not overwrite pending optimistic records with an earlier post-write snapshot", async () => {
  const files = new Map<string, unknown>();
  const held = Promise.withResolvers<unknown>();
  let reads = 0;
  const { collection, backend } = setup(async (request) => {
    if (request.type === "write") {
      files.set(
        request.path,
        JSON.parse(
          new TextDecoder().decode(new Uint8Array(request.content || [])),
        ),
      );
      return { type: "written", entry: entry(request.path) };
    }
    if (request.type === "list")
      return { type: "listed", entries: [...files.keys()].map(entry) };
    if (request.type === "read") {
      if (++reads === 1) return held.promise;
      return content(files.get(request.path));
    }
    return { type: "error", code: "operationFailed" };
  });
  await collection.preload();
  const first = collection.insert(record);
  await backend.waitForWrite?.();
  const second = collection.insert({ ...record, id: "two", name: "Pending" });
  expect(collection.get("two")?.name).toBe("Pending");
  held.resolve(content(record));
  await first.isPersisted.promise;
  expect(collection.get("two")?.name).toBe("Pending");
  await second.isPersisted.promise;
  expect([...collection.keys()]).toEqual(["one", "two"]);
});

it.each(["insert", "update", "delete"] as const)(
  "rolls back a rejected %s only after authoritative reconciliation",
  async (operation) => {
    const held = Promise.withResolvers<{
      type: string;
      entries?: unknown[];
    }>();
    let reconcile = false;
    const { collection } = setup(async (request) => {
      if (request.type === "write" || request.type === "delete") {
        reconcile = true;
        return { type: "error", code: "operationFailed" };
      }
      if (reconcile) return held.promise;
      if (request.type === "list")
        return {
          type: "listed",
          entries: operation === "insert" ? [] : [entry("secrets/one.json")],
        };
      return content(record);
    });
    await collection.preload();
    const tx =
      operation === "insert"
        ? collection.insert(record)
        : operation === "update"
          ? collection.update("one", (draft) => {
              draft.name = "Optimistic";
            })
          : collection.delete("one");
    const settled = tx.isPersisted.promise;
    await vi.waitFor(() => expect(reconcile).toBe(true));
    expect(tx.state).toBe("persisting");
    held.resolve({ type: "listed", entries: [] });
    await expect(settled).rejects.toThrow(
      "rejected; authoritative records reloaded",
    );
    expect(tx.state).toBe("failed");
    expect(collection.size).toBe(0);
    expect(collection.status).toBe("ready");
  },
);
