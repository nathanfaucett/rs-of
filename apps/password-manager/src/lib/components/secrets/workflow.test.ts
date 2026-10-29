import { StorageClient } from "@aicacia/storage-client";
import { flushSync } from "svelte";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
    Endpoint,
    entry,
    required,
    resetTransport,
    secret,
} from "./transport.fixture";
import { createSecretWorkflow } from "./workflow.svelte";

let workflow: ReturnType<typeof createSecretWorkflow>;
let client: StorageClient;
const exchange = vi.fn();

beforeEach(() => {
    resetTransport();
    vi.stubGlobal("WebSocket", Endpoint);
    exchange
        .mockReset()
        .mockImplementation(async () =>
            Response.json({ access_token: "storage-token" }),
        );
    client = new StorageClient({
        idpBaseUrl: "https://idp.test/idp",
        storageBaseUrl: "https://storage.test/storage",
        clientId: "password-manager-web",
        bearerToken: () => "user-token",
        fetch: exchange,
    });
    workflow = createSecretWorkflow({
        client,
        userId: "user-one",
        filesystemId: "first",
    });
});
afterEach(() => {
    workflow?.dispose();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
});
const settled = async () =>
    vi.waitFor(() => {
        flushSync();
        expect(workflow.loading).toBe(false);
    });

it("loads ordered records through the live query and cancels an editor without changing records", async () => {
    expect(workflow.loading).toBe(true);
    await settled();
    expect(workflow.records.map((value) => value.name)).toEqual([
        "First secret",
    ]);
    workflow.edit(workflow.records[0]);
    expect(workflow.editor?.record?.name).toBe("First secret");
    workflow.cancel();
    expect(workflow.editor).toBeNull();
    expect(workflow.records).toHaveLength(1);
});

it("validates before writing and preserves secret metadata while changing only the first website and secret history", async () => {
    await settled();
    const original = required(workflow.records[0]);
    workflow.edit(original);
    const draft = {
        name: "Renamed",
        secret: "changed",
        uri: "https://edited.test",
        notes: "",
        favorite: true,
    };
    await workflow.save({ ...draft, name: "" });
    expect(workflow.error).toContain("Invalid secret");
    expect(
        Endpoint.sockets[0]?.requests.filter(
            (request) => request.type === "write",
        ),
    ).toHaveLength(0);
    await workflow.save(draft);
    flushSync();
    expect(workflow.notice).toContain("Saved.");
    expect(workflow.records[0]).toMatchObject({
        ...secret,
        name: "Renamed",
        secret: "changed",
        favorite: true,
        uris: [
            { uri: "https://edited.test", match: "exact" },
            original.uris[1],
        ],
        updatedAt: expect.any(String),
        history: [
            ...original.history,
            { secret: "private", changedAt: expect.any(String) },
        ],
    });
    expect(workflow.records[0]?.updatedAt).not.toBe(original.updatedAt);
    expect(workflow.records[0]).toMatchObject(
        required(workflow.editor?.record),
    );
    await workflow.save({ ...draft, uri: "", notes: "Notes only" });
    flushSync();
    expect(workflow.records[0]?.uris).toEqual([original.uris[1]]);
    expect(workflow.records[0]?.history).toHaveLength(2);
});

it("optimistically creates and deletes but permits no conflicting actions or success until authoritative settlement", async () => {
    await settled();
    workflow.edit();
    await workflow.save({
        name: "New entry",
        secret: "new private",
        uri: "",
        notes: "",
        favorite: false,
    });
    flushSync();
    const created = required(
        workflow.records.find((record) => record.name === "New entry"),
    );
    expect(created).toMatchObject({
        uris: [],
        fields: [],
        history: [],
        favorite: false,
    });
    expect(workflow.notice).toContain("Saved.");
    Endpoint.holdWrites = true;
    const pending = workflow.delete(created);
    flushSync();
    expect(workflow.records.map((record) => record.name)).toEqual([
        "First secret",
    ]);
    expect(workflow.canEdit).toBe(false);
    expect(workflow.canRefresh).toBe(false);
    expect(workflow.notice).toBe("");
    workflow.edit();
    workflow.cancel();
    expect(workflow.editor?.record?.id).toBe(created.id);
    await vi.waitFor(() =>
        expect(Endpoint.sockets[0]?.requests.at(-1)?.type).toBe("delete"),
    );
    Endpoint.files.get("first")?.delete(`secrets/${created.id}.json`);
    Endpoint.sockets[0]?.dispatchEvent(
        new MessageEvent("message", {
            data: JSON.stringify({ type: "deleted" }),
        }),
    );
    await pending;
    flushSync();
    expect(workflow.canEdit).toBe(true);
    expect(workflow.editor).toBeNull();
    expect(workflow.notice).toContain("Deleted.");
});

it("refreshes external additions, updates and deletions in favorite/name order on the healthy socket and closes stale edits", async () => {
    await settled();
    workflow.edit(workflow.records[0]);
    const files = required(Endpoint.files.get("first"));
    files.set(`secrets/${secret.id}.json`, {
        ...secret,
        name: "Zulu",
        favorite: true,
    });
    files.set("secrets/01900000-0000-7000-8000-000000000002.json", {
        ...secret,
        id: "01900000-0000-7000-8000-000000000002",
        name: "Alpha",
    });
    files.set("secrets/01900000-0000-7000-8000-000000000003.json", {
        ...secret,
        id: "01900000-0000-7000-8000-000000000003",
        name: "Beta",
        favorite: true,
    });
    await workflow.refresh();
    flushSync();
    expect(workflow.records.map((record) => record.name)).toEqual([
        "Beta",
        "Zulu",
        "Alpha",
    ]);
    expect(workflow.editor).toBeNull();
    expect(Endpoint.sockets).toHaveLength(1);
    files.delete(`secrets/${secret.id}.json`);
    await workflow.refresh();
    flushSync();
    expect(workflow.records.map((record) => record.name)).toEqual([
        "Beta",
        "Alpha",
    ]);
    expect(workflow.notice).toContain("Refreshed.");
});

it("hides untrusted records and retires the editor on disconnect, recovering only on explicit Refresh", async () => {
    await settled();
    workflow.edit(workflow.records[0]);
    Endpoint.sockets[0]?.close();
    flushSync();
    expect(workflow.loadError).toContain("verify records before retrying");
    expect(workflow.records).toEqual([]);
    expect(workflow.editor).toBeNull();
    expect(workflow.canEdit).toBe(false);
    expect(Endpoint.sockets).toHaveLength(1);
    await workflow.refresh();
    flushSync();
    expect(workflow.loadError).toBeNull();
    expect(workflow.records[0]?.name).toBe("First secret");
    expect(Endpoint.sockets).toHaveLength(2);
});

it.each(["create", "update", "delete"] as const)(
    "reconciles rejected %s without success and retains the editable selection",
    async (operation) => {
        await settled();
        workflow.edit(operation === "create" ? null : workflow.records[0]);
        const selection = workflow.editor;
        Endpoint.rejectWrites = true;
        const draft = {
            name: "Rejected edit",
            secret: "private",
            uri: "",
            notes: "",
            favorite: false,
        };
        if (operation === "delete")
            await workflow.delete(required(workflow.records[0]));
        else await workflow.save(draft);
        flushSync();
        expect(workflow.error).toContain("Storage write rejected");
        expect(workflow.records.map((record) => record.name)).toEqual([
            "First secret",
        ]);
        expect(workflow.editor).toBe(selection);
        expect(workflow.canEdit).toBe(true);
        expect(workflow.notice).toBe("");
    },
);

it.each(["create", "update", "delete"] as const)(
    "shows the authoritative result of an uncertain %s, closes its draft and never reports success",
    async (operation) => {
        await settled();
        workflow.edit(operation === "create" ? null : workflow.records[0]);
        Endpoint.ambiguousWrites = true;
        if (operation === "delete")
            await workflow.delete(required(workflow.records[0]));
        else
            await workflow.save({
                name: "Possibly saved",
                secret: "private",
                uri: "",
                notes: "",
                favorite: false,
            });
        flushSync();
        expect(workflow.error).toContain(
            "not acknowledged; authoritative records reloaded",
        );
        expect(workflow.records.map((record) => record.name)).toEqual(
            operation === "delete"
                ? []
                : operation === "create"
                  ? ["First secret", "Possibly saved"]
                  : ["Possibly saved"],
        );
        expect(workflow.editor).toBeNull();
        expect(workflow.notice).toBe("");
        const writes = required(Endpoint.sockets[0]).requests.filter(
            (request) => request.type === "write",
        ).length;
        await workflow.save({
            name: "Blind retry",
            secret: "private",
            uri: "",
            notes: "",
            favorite: false,
        });
        expect(
            required(Endpoint.sockets[0]).requests.filter(
                (request) => request.type === "write",
            ),
        ).toHaveLength(writes);
    },
);

it("never publishes partial malformed Refresh results and recovers only after repair and explicit retry", async () => {
    await settled();
    workflow.edit(workflow.records[0]);
    required(Endpoint.files.get("first")).set("secrets/broken.json", {
        invalid: true,
    });
    await workflow.refresh();
    flushSync();
    expect(workflow.loadError).toContain("Malformed Storage record");
    expect(workflow.records).toEqual([]);
    expect(workflow.canEdit).toBe(false);
    expect(workflow.editor).toBeNull();
    expect(workflow.notice).toBe("");
    expect(Endpoint.sockets).toHaveLength(1);
    required(Endpoint.files.get("first")).delete("secrets/broken.json");
    await workflow.refresh();
    flushSync();
    expect(workflow.records[0]?.name).toBe("First secret");
    expect(workflow.loadError).toBeNull();
    expect(Endpoint.sockets).toHaveLength(2);
});

it("hides records after an acknowledged write whose reconciliation fails and requires Refresh before any retry", async () => {
    await settled();
    workflow.edit();
    required(Endpoint.files.get("first")).set("secrets/broken.json", {
        invalid: true,
    });
    await workflow.save({
        name: "Applied but untrusted",
        secret: "private",
        uri: "",
        notes: "",
        favorite: false,
    });
    flushSync();
    expect(workflow.loadError).toContain(
        "Refresh and verify records before retrying",
    );
    expect(workflow.error).toContain("could not be reconciled");
    expect(workflow.records).toEqual([]);
    expect(workflow.editor).toBeNull();
    expect(workflow.notice).toBe("");
    expect(workflow.canEdit).toBe(false);
    const requests = required(Endpoint.sockets[0]).requests.length;
    workflow.edit();
    await workflow.save({
        name: "Blind retry",
        secret: "private",
        uri: "",
        notes: "",
        favorite: false,
    });
    expect(required(Endpoint.sockets[0]).requests).toHaveLength(requests);
    required(Endpoint.files.get("first")).delete("secrets/broken.json");
    await workflow.refresh();
    flushSync();
    expect(workflow.records.map((record) => record.name)).toEqual([
        "Applied but untrusted",
        "First secret",
    ]);
    expect(workflow.editor).toBeNull();
});

it.each(["save", "delete"] as const)(
    "retires records, selection and notices immediately when disposed during a pending %s",
    async (operation) => {
        await settled();
        workflow.edit(workflow.records[0]);
        Endpoint.holdWrites = true;
        const pending =
            operation === "save"
                ? workflow.save({
                      name: "Late result",
                      secret: "private",
                      uri: "",
                      notes: "",
                      favorite: false,
                  })
                : workflow.delete(required(workflow.records[0]));
        await vi.waitFor(() =>
            expect(required(Endpoint.sockets[0]).requests.at(-1)?.type).toBe(
                operation === "save" ? "write" : "delete",
            ),
        );
        const old = required(Endpoint.sockets[0]);
        workflow.dispose();
        expect(workflow.records).toEqual([]);
        expect(workflow.editor).toBeNull();
        expect(workflow.notice).toBe("");
        expect(workflow.canEdit).toBe(false);
        expect(workflow.canRefresh).toBe(false);
        expect(old.closed).toBe(true);
        old.dispatchEvent(
            new MessageEvent("message", {
                data: JSON.stringify({ type: "deleted" }),
            }),
        );
        await pending;
        expect(workflow.error).toBeNull();
        expect(workflow.notice).toBe("");
    },
);

it.each(["load", "refresh"] as const)(
    "ignores late %s results after disposal without populating a replacement workflow",
    async (operation) => {
        if (operation === "refresh") {
            await settled();
            workflow.edit(workflow.records[0]);
        }
        const send = Endpoint.prototype.send;
        vi.spyOn(Endpoint.prototype, "send").mockImplementation(function (
            this: Endpoint,
            data: string,
        ) {
            if (
                new URL(this.url).searchParams.get("filesystem_id") === "first"
            ) {
                this.requests.push(JSON.parse(data));
                return;
            }
            send.call(this, data);
        });
        const pending =
            operation === "refresh" ? workflow.refresh() : Promise.resolve();
        await vi.waitFor(() =>
            expect(Endpoint.sockets[0]?.requests.at(-1)?.type).toBe("list"),
        );
        const old = required(Endpoint.sockets[0]);
        const retired = workflow;
        retired.dispose();
        expect(retired.records).toEqual([]);
        expect(retired.editor).toBeNull();
        expect(retired.busy).toBe(false);
        expect(retired.refreshing).toBe(false);
        workflow = createSecretWorkflow({
            client,
            userId: "user-one",
            filesystemId: "empty",
        });
        await settled();
        old.dispatchEvent(
            new MessageEvent("message", {
                data: JSON.stringify({ type: "listed", entries: [] }),
            }),
        );
        await pending;
        flushSync();
        expect(old.closed).toBe(true);
        expect(workflow.records).toEqual([]);
        expect(workflow.loadError).toBeNull();
        expect(workflow.notice).toBe("");
        expect(retired.notice).toBe("");
        expect(retired.error).toBeNull();
        expect(retired.loadError).toBeNull();
    },
);

it("reports a partially malformed initial snapshot rather than an empty or incomplete filesystem", async () => {
    await settled();
    workflow.dispose();
    required(Endpoint.files.get("first")).set("secrets/broken.json", {
        invalid: true,
    });
    workflow = createSecretWorkflow({
        client,
        userId: "user-one",
        filesystemId: "first",
    });
    await settled();
    expect(workflow.loadError).toContain("Malformed Storage record");
    expect(workflow.records).toEqual([]);
    expect(workflow.canEdit).toBe(false);
    expect(workflow.canRefresh).toBe(true);
});

it("reports authorization failure without presenting an empty ready resource", async () => {
    await settled();
    workflow.dispose();
    exchange.mockResolvedValueOnce(new Response(null, { status: 401 }));
    workflow = createSecretWorkflow({
        client,
        userId: "user-one",
        filesystemId: "empty",
    });
    await settled();
    expect(workflow.loadError).toContain("Storage token exchange failed: 401");
    expect(workflow.records).toEqual([]);
    expect(workflow.canEdit).toBe(false);
    expect(workflow.canRefresh).toBe(true);
    expect(Endpoint.sockets).toHaveLength(1);
    await workflow.refresh();
    flushSync();
    expect(workflow.loadError).toBeNull();
    expect(workflow.canEdit).toBe(true);
    expect(workflow.records).toEqual([]);
});

it("hides records after a write disconnects without retrying or reporting success", async () => {
    await settled();
    workflow.edit(workflow.records[0]);
    Endpoint.holdWrites = true;
    const pending = workflow.save({
        name: "Unknown outcome",
        secret: "private",
        uri: "",
        notes: "",
        favorite: false,
    });
    await vi.waitFor(() =>
        expect(Endpoint.sockets[0]?.requests.at(-1)?.type).toBe("write"),
    );
    required(Endpoint.sockets[0]).close();
    await pending;
    flushSync();
    expect(workflow.loadError).toContain(
        "Refresh and verify records before retrying",
    );
    expect(workflow.records).toEqual([]);
    expect(workflow.editor).toBeNull();
    expect(workflow.notice).toBe("");
    expect(workflow.canEdit).toBe(false);
    expect(Endpoint.sockets).toHaveLength(1);
});

it.each(["create", "update"] as const)(
    "shows optimistic %s but waits for reconciliation before success or accepting more work",
    async (operation) => {
        await settled();
        workflow.edit(operation === "create" ? null : workflow.records[0]);
        Endpoint.holdWrites = true;
        const pending = workflow.save({
            name: "Pending edit",
            secret: "changed",
            uri: "",
            notes: "",
            favorite: false,
        });
        flushSync();
        expect(workflow.records.map((record) => record.name)).toContain(
            "Pending edit",
        );
        expect(workflow.canEdit).toBe(false);
        expect(workflow.canRefresh).toBe(false);
        expect(workflow.notice).toBe("");
        await vi.waitFor(() =>
            expect(required(Endpoint.sockets[0]).requests.at(-1)?.type).toBe(
                "write",
            ),
        );
        const socket = required(Endpoint.sockets[0]);
        const write = required(socket.requests.at(-1));
        if (write.type !== "write") throw new Error("Expected a held write");
        required(Endpoint.files.get("first")).set(
            write.path,
            JSON.parse(new TextDecoder().decode(new Uint8Array(write.content))),
        );
        const send = Endpoint.prototype.send;
        vi.spyOn(Endpoint.prototype, "send").mockImplementation(function (
            this: Endpoint,
            data: string,
        ) {
            if (JSON.parse(data).type === "list") {
                this.requests.push(JSON.parse(data));
                this.hold = true;
                return;
            }
            send.call(this, data);
        });
        socket.dispatchEvent(
            new MessageEvent("message", {
                data: JSON.stringify({
                    type: "written",
                    entry: entry(write.path),
                }),
            }),
        );
        await vi.waitFor(() =>
            expect(socket.requests.at(-1)?.type).toBe("list"),
        );
        expect(workflow.notice).toBe("");
        expect(workflow.canEdit).toBe(false);
        expect(workflow.canRefresh).toBe(false);
        vi.restoreAllMocks();
        socket.hold = false;
        socket.dispatchEvent(
            new MessageEvent("message", {
                data: JSON.stringify({
                    type: "listed",
                    entries: [entry("secrets")],
                }),
            }),
        );
        await pending;
        flushSync();
        expect(workflow.notice).toContain("Saved.");
        expect(workflow.canEdit).toBe(true);
        expect(workflow.editor?.record?.name).toBe("Pending edit");
    },
);
