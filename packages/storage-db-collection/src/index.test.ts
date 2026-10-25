import {
    StorageClient,
    type StorageRequest,
    type StorageResponse,
} from "@aicacia/storage-client";
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
const content = (value: unknown): StorageResponse => ({
    type: "read",
    content: [...new TextEncoder().encode(JSON.stringify(value))],
});

class Endpoint extends EventTarget {
    static OPEN = 1;
    static connecting = false;
    readyState = Endpoint.connecting ? 0 : 1;
    static sockets: Endpoint[] = [];
    requests: StorageRequest[] = [];
    closed = false;
    respond: (request: StorageRequest) => Promise<StorageResponse> =
        async () => ({ type: "listed", entries: [] });
    constructor(readonly url: string) {
        super();
        Endpoint.sockets.push(this);
    }
    send(data: string) {
        const request = JSON.parse(data) as StorageRequest;
        this.requests.push(request);
        void this.respond(request).then((response) =>
            this.dispatchEvent(
                new MessageEvent("message", { data: JSON.stringify(response) }),
            ),
        );
    }
    close() {
        this.closed = true;
        this.readyState = 3;
        this.dispatchEvent(new Event("close"));
    }
}
const collections: { cleanup(): Promise<void> }[] = [];
afterEach(async () => {
    for (const collection of collections.splice(0).reverse())
        await collection.cleanup();
    Endpoint.sockets = [];
    Endpoint.connecting = false;
    vi.unstubAllGlobals();
});

function setup(respond?: Endpoint["respond"]) {
    vi.stubGlobal(
        "WebSocket",
        class extends Endpoint {
            constructor(url: string) {
                super(url);
                this.respond = async (request) =>
                    request.type === "list" && request.path === ""
                        ? { type: "listed", entries: [entry("secrets")] }
                        : respond
                          ? respond(request)
                          : { type: "listed", entries: [] };
            }
        },
    );
    const fetchApi = vi.fn(async () =>
        Response.json({ access_token: "storage-token" }),
    );
    const client = new StorageClient({
        idpBaseUrl: "https://idp.test/prefix/idp",
        storageBaseUrl: "https://storage.test/prefix/storage",
        clientId: "test-app",
        bearerToken: () => "user-token",
        fetch: fetchApi,
    });
    const collection = createStorageCollection({
        client,
        userId: "user",
        applicationId: "app",
        filesystemId: "fs-one",
        directory: "secrets",
        schema,
        getKey: (value) => value.id,
    });
    collections.push(collection);
    return { collection, client, fetchApi };
}

it("loads validated records through Storage into a real ordered live query", async () => {
    const { collection, fetchApi } = setup(async (request) =>
        request.type === "list"
            ? {
                  type: "listed",
                  entries: [
                      entry("secrets/one.json"),
                      entry("secrets/two.json"),
                      entry("secrets/ignore.txt"),
                  ],
              }
            : content(
                  request.path === "secrets/one.json"
                      ? record
                      : { id: "two", name: "Zulu", favorite: true },
              ),
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
    expect([...query.values()].map((value) => value.id)).toEqual([
        "two",
        "one",
    ]);
    expect(collection.status).toBe("ready");
    expect(Endpoint.sockets[0]?.requests).toEqual([
        { type: "list", path: "" },
        { type: "list", path: "secrets" },
        { type: "read", path: "secrets/one.json" },
        { type: "read", path: "secrets/two.json" },
    ]);
    expect(Endpoint.sockets[0]?.url).toBe(
        "wss://storage.test/prefix/storage?access_token=storage-token&filesystem_id=fs-one",
    );
    expect(String(fetchApi.mock.calls[0]?.[0])).toBe(
        "https://idp.test/prefix/idp/oauth2/token",
    );
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
        const { collection } = setup(async () => ({
            type: "listed",
            entries: [entry(`secrets/${key}.json`)],
        }));
        await expect(collection.preload()).rejects.toThrow("Unsafe record key");
        expect(collection.status).toBe("error");
        expect(collection.size).toBe(0);
        expect(
            Endpoint.sockets[0]?.requests.every(
                (request) => request.type === "list",
            ),
        ).toBe(true);
    },
);

it.each([
    { type: "error", code: "operationFailed" },
    { type: "read", content: [123] },
    content({ ...record, favorite: "invalid" }),
    content({ ...record, id: "other" }),
] satisfies StorageResponse[])(
    "never publishes a partial or malformed snapshot (%j)",
    async (response) => {
        const { collection } = setup(async (request) =>
            request.type === "list"
                ? {
                      type: "listed",
                      entries: [
                          entry("secrets/one.json"),
                          entry("secrets/two.json"),
                      ],
                  }
                : request.path.endsWith("one.json")
                  ? content(record)
                  : response,
        );
        await expect(collection.preload()).rejects.toThrow();
        expect(collection.status).toBe("error");
        expect(collection.size).toBe(0);
        expect(Endpoint.sockets[0]?.closed).toBe(true);
    },
);

it("surfaces denied authorization without opening a socket", async () => {
    const { collection, fetchApi } = setup();
    fetchApi.mockResolvedValueOnce(new Response(null, { status: 403 }));
    await expect(collection.preload()).rejects.toThrow("403");
    expect(collection.status).toBe("error");
    expect(Endpoint.sockets).toHaveLength(0);
});

it("disposes during a held listing without reading or publishing late records", async () => {
    const held = Promise.withResolvers<StorageResponse>();
    const { collection } = setup(() => held.promise);
    const loading = collection.preload().catch(() => undefined);
    await vi.waitFor(() =>
        expect(Endpoint.sockets[0]?.requests).toHaveLength(2),
    );
    const endpoint = Endpoint.sockets[0];
    await collection.cleanup();
    held.resolve({ type: "listed", entries: [entry("secrets/one.json")] });
    await loading;
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(collection.size).toBe(0);
    expect(endpoint?.closed).toBe(true);
    expect(endpoint?.requests).toHaveLength(2);
    await expect(collection.preload()).rejects.toThrow("disposed");
    expect(Endpoint.sockets).toHaveLength(1);
});

it("closes a socket that opens after disposal without sending a request", async () => {
    const { collection, fetchApi } = setup();
    const token = Promise.withResolvers<Response>();
    fetchApi.mockReturnValueOnce(token.promise);
    const loading = collection.preload().catch(() => undefined);
    await collection.cleanup();
    token.resolve(Response.json({ access_token: "late" }));
    await loading;
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(Endpoint.sockets).toHaveLength(0);
    expect(collection.size).toBe(0);
});

it("does not initiate work if disposed before loading", async () => {
    const { collection, fetchApi } = setup();
    await collection.cleanup();
    await expect(collection.preload()).rejects.toThrow("disposed");
    expect(fetchApi).not.toHaveBeenCalled();
});

it("closes a still-connecting socket immediately on disposal", async () => {
    const { collection } = setup();
    Endpoint.connecting = true;
    const loading = collection.preload().catch(() => undefined);
    await vi.waitFor(() => expect(Endpoint.sockets).toHaveLength(1));
    await collection.cleanup();
    expect(Endpoint.sockets[0]?.closed).toBe(true);
    await loading;
});

it("does not publish a validated record if the connection closes during validation", async () => {
    const { client } = setup(async (request) =>
        request.type === "list"
            ? { type: "listed", entries: [entry("secrets/one.json")] }
            : content(record),
    );
    const validation = Promise.withResolvers<{ value: typeof record }>();
    const validating = vi.fn(() => validation.promise);
    const collection = createStorageCollection({
        client,
        userId: "user",
        applicationId: "app",
        filesystemId: "fs",
        directory: "secrets",
        getKey: (value: typeof record) => value.id,
        schema: {
            "~standard": { version: 1, vendor: "test", validate: validating },
        },
    });
    collections.push(collection);
    const loading = collection.preload().catch(() => undefined);
    await vi.waitFor(() => expect(validating).toHaveBeenCalledOnce());
    Endpoint.sockets[0]?.close();
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
    const { client, fetchApi } = setup();
    expect(() =>
        createStorageCollection({
            client,
            userId: "user",
            applicationId: "app",
            filesystemId: "fs",
            directory,
            schema,
            getKey: (value) => value.id,
        }),
    ).toThrow("Unsafe record directory");
    expect(fetchApi).not.toHaveBeenCalled();
});

it("cannot perform a legacy write through the read collection", async () => {
    const { collection } = setup();
    await collection.preload();
    expect(() => collection.insert(record)).toThrow();
    expect(
        Endpoint.sockets[0]?.requests.every(
            (request) => request.type === "list",
        ),
    ).toBe(true);
});

it("does not exchange a late bearer token after disposal", async () => {
    const { fetchApi } = setup();
    const bearer = Promise.withResolvers<string>();
    const client = new StorageClient({
        idpBaseUrl: "https://idp.test/idp",
        storageBaseUrl: "https://storage.test/storage",
        clientId: "test-app",
        bearerToken: () => bearer.promise,
        fetch: fetchApi,
    });
    const collection = createStorageCollection({
        client,
        userId: "user",
        applicationId: "app",
        filesystemId: "fs",
        directory: "secrets",
        schema,
        getKey: (value) => value.id,
    });
    collections.push(collection);
    const loading = collection.preload().catch(() => undefined);
    await collection.cleanup();
    bearer.resolve("late-user-token");
    await loading;
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(fetchApi).not.toHaveBeenCalled();
    expect(Endpoint.sockets).toHaveLength(0);
});
