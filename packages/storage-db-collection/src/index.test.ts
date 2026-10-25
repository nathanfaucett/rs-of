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

it("optimistically inserts but settles only after authoritative reconciliation, then reopens persisted CRUD", async () => {
    const files = new Map<string, unknown>();
    const held = Promise.withResolvers<StorageResponse>();
    let hold = true;
    const respond: Endpoint["respond"] = async (request) => {
        if (request.type === "write") {
            files.set(
                request.path,
                JSON.parse(
                    new TextDecoder().decode(new Uint8Array(request.content)),
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
        if (hold) return held.promise;
        return content(files.get(request.path));
    };
    const { collection } = setup(respond);
    await collection.preload();
    const tx = collection.insert(record);
    expect(collection.get("one")).toMatchObject(record);
    await vi.waitFor(() =>
        expect(
            Endpoint.sockets[0]?.requests.some(
                (request) => request.type === "read",
            ),
        ).toBe(true),
    );
    expect(tx.state).toBe("persisting");
    hold = false;
    // The server normalizes a field: the authoritative record must win.
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
    const held = Promise.withResolvers<StorageResponse>();
    let reads = 0;
    const { collection } = setup(async (request) => {
        if (request.type === "write") {
            files.set(
                request.path,
                JSON.parse(
                    new TextDecoder().decode(new Uint8Array(request.content)),
                ),
            );
            return { type: "written", entry: entry(request.path) };
        }
        if (request.type === "list")
            return { type: "listed", entries: [...files.keys()].map(entry) };
        if (++reads === 1) return held.promise;
        return content(files.get(request.path));
    });
    await collection.preload();
    const first = collection.insert(record);
    await vi.waitFor(() => expect(reads).toBe(1));
    const second = collection.insert({ ...record, id: "two", name: "Pending" });
    expect(collection.get("two")?.name).toBe("Pending");
    held.resolve(content(record));
    await first.isPersisted.promise;
    expect(collection.get("two")?.name).toBe("Pending");
    await second.isPersisted.promise;
    expect([...collection.keys()]).toEqual(["one", "two"]);
    expect(
        Endpoint.sockets[0]?.requests
            .filter((request) => request.type === "write")
            .map((request) => request.path),
    ).toEqual(["secrets/one.json", "secrets/two.json"]);
});

it.each(["insert", "update", "delete"] as const)(
    "rolls back a rejected %s only after authoritative reconciliation",
    async (operation) => {
        const held = Promise.withResolvers<StorageResponse>();
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
                    entries:
                        operation === "insert"
                            ? []
                            : [entry("secrets/one.json")],
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
        // Reconciliation is authoritative even if another client deleted the record.
        expect(collection.size).toBe(0);
        expect(collection.status).toBe("ready");
    },
);

it("never sends a queued mutation explicitly rolled back before transport", async () => {
    const held = Promise.withResolvers<StorageResponse>();
    const { collection } = setup(async (request) => {
        if (request.type === "write") return held.promise;
        return { type: "listed", entries: [] };
    });
    await collection.preload();
    const first = collection.insert(record);
    await vi.waitFor(() =>
        expect(Endpoint.sockets[0]?.requests.at(-1)?.type).toBe("write"),
    );
    const second = collection.update("one", (draft) => {
        draft.name = "Dependent edit";
    });
    second.rollback({ error: new Error("Cancelled edit") });
    held.resolve({ type: "error", code: "operationFailed" });
    await expect(first.isPersisted.promise).rejects.toThrow("rejected");
    await expect(second.isPersisted.promise).rejects.toThrow("Cancelled edit");
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(
        Endpoint.sockets[0]?.requests.filter(
            (request) => request.type === "write",
        ),
    ).toHaveLength(1);
    expect(collection.size).toBe(0);
});

it.each(["unexpected response", "wrong acknowledgement path"])(
    "reconciles an unacknowledged but applied write without claiming success (%s)",
    async (failure) => {
        let saved = false;
        const { collection } = setup(async (request) => {
            if (request.type === "write") {
                saved = true;
                return failure === "unexpected response"
                    ? { type: "authenticated" }
                    : { type: "written", entry: entry("secrets/other.json") };
            }
            if (request.type === "list")
                return {
                    type: "listed",
                    entries: saved ? [entry("secrets/one.json")] : [],
                };
            return content(record);
        });
        await collection.preload();
        await expect(
            collection.insert(record).isPersisted.promise,
        ).rejects.toThrow("not acknowledged; authoritative records reloaded");
        expect(collection.get("one")).toMatchObject(record);
        expect(collection.status).toBe("ready");
    },
);

it.each(["disconnect", "malformed"])(
    "fails closed when an applied write cannot be reconciled (%s)",
    async (failure) => {
        let saved = false;
        const { collection } = setup(async (request) => {
            if (request.type === "write") {
                saved = true;
                if (failure === "disconnect") Endpoint.sockets[0]?.close();
                return { type: "written", entry: entry(request.path) };
            }
            if (request.type === "list")
                return {
                    type: "listed",
                    entries: saved ? [entry("secrets/one.json")] : [],
                };
            return content({ invalid: true });
        });
        await collection.preload();
        await expect(
            collection.insert(record).isPersisted.promise,
        ).rejects.toThrow(/reopen|Reopen/);
        expect(saved).toBe(true);
        expect(collection.status).toBe("error");
        expect(Endpoint.sockets[0]?.closed).toBe(true);
        const count = Endpoint.sockets[0]?.requests.length;
        expect(() => collection.insert({ ...record, id: "other" })).toThrow();
        expect(Endpoint.sockets[0]?.requests.length).toBe(count);
    },
);

it.each(["../escape", "nested/key", "", "%2e%2e"])(
    "rejects outgoing unsafe key %s without a Storage write",
    async (id) => {
        const { collection } = setup();
        await collection.preload();
        await expect(
            collection.insert({ ...record, id }).isPersisted.promise,
        ).rejects.toThrow("Unsafe record key");
        expect(collection.size).toBe(0);
        expect(
            Endpoint.sockets[0]?.requests.every(
                (request) => request.type === "list",
            ),
        ).toBe(true);
    },
);

it("validates outgoing records and refuses multi-record atomicity before transport", async () => {
    const { collection } = setup();
    await collection.preload();
    await expect(
        collection.insert({
            ...record,
            favorite: "invalid" as unknown as boolean,
        }).isPersisted.promise,
    ).rejects.toThrow("Malformed Storage record");
    await expect(
        collection.insert([record, { ...record, id: "two" }]).isPersisted
            .promise,
    ).rejects.toThrow("single-record");
    expect(collection.size).toBe(0);
    expect(
        Endpoint.sockets[0]?.requests.every(
            (request) => request.type === "list",
        ),
    ).toBe(true);
});

it("rejects serialization failure without sending a write or exposing secret content", async () => {
    const { client } = setup();
    const collection = createStorageCollection({
        client,
        userId: "user",
        applicationId: "app",
        filesystemId: "fs",
        directory: "secrets",
        schema,
        getKey: (value) => value.id,
        serialize: () => {
            throw new Error("private record contents");
        },
    });
    collections.push(collection);
    await collection.preload();
    await expect(collection.insert(record).isPersisted.promise).rejects.toThrow(
        /^Storage record serialization failed$/,
    );
    expect(collection.size).toBe(0);
    expect(collection.status).toBe("ready");
    expect(
        Endpoint.sockets[0]?.requests.every(
            (request) => request.type === "list",
        ),
    ).toBe(true);
});

it("isolates disposal during a sent write and prevents queued writes or late reconciliation in a replacement scope", async () => {
    const held = Promise.withResolvers<StorageResponse>();
    const { collection, client } = setup(async (request) =>
        request.type === "write"
            ? held.promise
            : { type: "listed", entries: [] },
    );
    await collection.preload();
    const tx = collection.insert(record);
    const queued = collection.insert({ ...record, id: "two" });
    await vi.waitFor(() =>
        expect(
            Endpoint.sockets[0]?.requests.some(
                (request) => request.type === "write",
            ),
        ).toBe(true),
    );
    const old = Endpoint.sockets[0];
    if (!old) throw new Error("Expected the original Storage socket");
    await collection.cleanup();
    const replacement = createStorageCollection({
        client,
        userId: "other-user",
        applicationId: "app",
        filesystemId: "other-fs",
        directory: "secrets",
        schema,
        getKey: (value) => value.id,
    });
    collections.push(replacement);
    await replacement.preload();
    held.resolve({ type: "written", entry: entry("secrets/one.json") });
    await expect(tx.isPersisted.promise).rejects.toThrow("disposed");
    await expect(queued.isPersisted.promise).rejects.toThrow("disposed");
    expect(old.closed).toBe(true);
    expect(
        old.requests.filter((request) => request.type === "write"),
    ).toHaveLength(1);
    expect(old.requests.at(-1)?.type).toBe("write");
    expect(replacement.size).toBe(0);
    expect(collection.size).toBe(0);
});

it("does not send a write after disposal during outgoing asynchronous validation", async () => {
    const { client } = setup();
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
    await collection.preload();
    const tx = collection.insert(record);
    await vi.waitFor(() => expect(validating).toHaveBeenCalledOnce());
    await collection.cleanup();
    validation.resolve({ value: record });
    await expect(tx.isPersisted.promise).rejects.toThrow("disposed");
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
