import { afterEach, describe, expect, it, vi } from "vitest";

import { StorageClient } from "./index";

afterEach(() => vi.unstubAllGlobals());

class TestSocket extends EventTarget {
    static OPEN = 1;
    readyState = 1;
    constructor(readonly url: string) {
        super();
    }
    send = vi.fn((message: string) => {
        const request = JSON.parse(message);
        const response =
            request.type === "read"
                ? { type: "read", content: [42] }
                : request.type === "write"
                  ? {
                        type: "written",
                        entry: {
                            name: "secret.json",
                            hash: "hash",
                            size: 1,
                            local: true,
                        },
                    }
                  : { type: "error", code: "invalidRequest" };
        queueMicrotask(() =>
            this.dispatchEvent(
                new MessageEvent("message", { data: JSON.stringify(response) }),
            ),
        );
    });
    close = vi.fn();
}

describe("StorageClient", () => {
    it.each(["list", "create"])(
        "%s cancellation while obtaining the bearer sends no exchange or resource request",
        async (operation) => {
            let release!: (token: string) => void;
            const token = new Promise<string>((resolve) => {
                release = resolve;
            });
            const bearerToken = vi.fn(() => token);
            const fetch = vi.fn();
            const client = new StorageClient({
                idpBaseUrl: "https://idp.example/idp",
                storageBaseUrl: "https://storage.example/storage",
                clientId: "password-manager-web",
                bearerToken,
                fetch,
            });
            const controller = new AbortController();
            const pending =
                operation === "list"
                    ? client.listFileSystems(controller.signal)
                    : client.createFileSystem("Example", controller.signal);
            expect(bearerToken).toHaveBeenCalledOnce();
            controller.abort();
            release("obsolete-user-token");
            await expect(pending).rejects.toMatchObject({ name: "AbortError" });
            expect(fetch).not.toHaveBeenCalled();
        },
    );

    it("passes cancellation to exchange and resource requests without pretending to undo an already-sent creation", async () => {
        let release!: (response: Response) => void;
        const held = new Promise<Response>((resolve) => {
            release = resolve;
        });
        const fetch = vi
            .fn()
            .mockResolvedValueOnce(
                Response.json({ access_token: "storage-token" }),
            )
            .mockReturnValueOnce(held);
        const client = new StorageClient({
            idpBaseUrl: "https://idp.example/idp",
            storageBaseUrl: "https://storage.example/storage",
            clientId: "password-manager-web",
            bearerToken: () => "user-token",
            fetch,
        });
        const controller = new AbortController();
        const pending = client.createFileSystem("Example", controller.signal);
        await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
        expect(fetch.mock.calls.map((call) => call[1].signal)).toEqual([
            controller.signal,
            controller.signal,
        ]);
        controller.abort();
        release(Response.json({ id: "created", name: "Example" }));
        await expect(pending).resolves.toEqual({
            id: "created",
            name: "Example",
        });
        expect(fetch).toHaveBeenCalledTimes(2);
    });
    it.each([
        {
            name: "missing-user",
            subject: "",
            response: new Response(null),
            message: "Missing OIDC access token",
        },
        {
            name: "denied-exchange",
            subject: "user-token",
            response: new Response(null, { status: 403 }),
            message: "Storage token exchange failed: 403",
        },
        {
            name: "missing-issued-token",
            subject: "user-token",
            response: Response.json({}),
            message: "Storage token exchange returned no access token",
        },
    ])(
        "does not request a catalog or socket after $name",
        async ({ subject, response, message }) => {
            const fetch = vi
                .fn()
                .mockImplementation(async () => response.clone());
            const WebSocket = vi.fn();
            vi.stubGlobal("WebSocket", WebSocket);
            const client = new StorageClient({
                idpBaseUrl: "https://idp.example/idp",
                storageBaseUrl: "https://storage.example/storage",
                clientId: "password-manager-web",
                bearerToken: () => subject,
                fetch,
            });
            await expect(client.listFileSystems()).rejects.toThrow(message);
            await expect(
                client.openSocket("01900000-0000-7000-8000-000000000001"),
            ).rejects.toThrow(message);
            expect(
                fetch.mock.calls.map(([url]) => new URL(String(url)).href),
            ).toEqual(
                subject
                    ? [
                          "https://idp.example/idp/oauth2/token",
                          "https://idp.example/idp/oauth2/token",
                      ]
                    : [],
            );
            expect(WebSocket).not.toHaveBeenCalled();
        },
    );

    it.each([
        "ftp://idp.example/idp",
        "https://user:password@idp.example/idp",
        "https://idp.example/idp?query=true",
        "https://idp.example/idp#fragment",
    ])(
        "rejects invalid service base %s without transmitting the bearer",
        async (idpBaseUrl) => {
            const fetch = vi.fn();
            const client = new StorageClient({
                idpBaseUrl,
                storageBaseUrl: "https://storage.example/storage",
                clientId: "password-manager-web",
                bearerToken: () => "user-token",
                fetch,
            });
            await expect(client.listFileSystems()).rejects.toThrow();
            expect(fetch).not.toHaveBeenCalled();
        },
    );

    it("surfaces current catalog authorization denial instead of claiming an empty catalog", async () => {
        const fetch = vi
            .fn()
            .mockResolvedValueOnce(
                Response.json({ access_token: "read-token" }),
            )
            .mockResolvedValueOnce(new Response(null, { status: 403 }));
        const client = new StorageClient({
            idpBaseUrl: "https://idp.example/idp",
            storageBaseUrl: "https://storage.example/storage",
            clientId: "password-manager-web",
            bearerToken: () => "user-token",
            fetch,
        });
        await expect(client.listFileSystems()).rejects.toThrow(
            "Filesystem list failed: 403",
        );
        expect(fetch).toHaveBeenCalledTimes(2);
    });
    it.each(["", "/"])(
        "uses write-only filesystem creation with service-base trailing slash %s",
        async (slash) => {
            const fetch = vi
                .fn()
                .mockResolvedValueOnce(
                    Response.json({ access_token: "write-token" }),
                )
                .mockResolvedValueOnce(
                    Response.json(
                        { id: "filesystem-id", name: "Vault" },
                        { status: 201 },
                    ),
                );
            const client = new StorageClient({
                idpBaseUrl: new URL(
                    `https://idp.example/deployment/idp${slash}`,
                ),
                storageBaseUrl: new URL(
                    `https://storage.example/deployment/storage${slash}`,
                ),
                clientId: "password-manager-web",
                bearerToken: () => "user-token",
                fetch,
            });
            expect(await client.createFileSystem("Vault")).toEqual({
                id: "filesystem-id",
                name: "Vault",
            });
            expect(
                fetch.mock.calls.map(([url]) => new URL(String(url)).href),
            ).toEqual([
                "https://idp.example/deployment/idp/oauth2/token",
                "https://storage.example/deployment/storage/filesystems",
            ]);
            const body = fetch.mock.calls[0]?.[1]?.body as URLSearchParams;
            expect(body.get("resource")).toBe("storage:filesystems");
            expect(
                JSON.parse(body.get("authorization_details") ?? "null"),
            ).toEqual([{ type: "storage", actions: ["write"] }]);
            expect(fetch.mock.calls[1]?.[1]).toEqual({
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    Authorization: "Bearer write-token",
                },
                body: '{"name":"Vault"}',
            });
        },
    );

    it("uses the Storage socket base and sends current read/write requests without a legacy envelope", async () => {
        vi.stubGlobal("WebSocket", TestSocket);
        const fetch = vi
            .fn()
            .mockResolvedValue(Response.json({ access_token: "socket-token" }));
        const client = new StorageClient({
            idpBaseUrl: "https://idp.example/idp",
            storageBaseUrl: "https://storage.example/storage/",
            clientId: "password-manager-web",
            bearerToken: () => "user-token",
            fetch,
        });
        const socket = await client.openSocket(
            "01900000-0000-7000-8000-000000000001",
        );
        expect((socket.socket as unknown as TestSocket).url).toBe(
            "wss://storage.example/storage?access_token=socket-token&filesystem_id=01900000-0000-7000-8000-000000000001",
        );
        expect(String(fetch.mock.calls[0]?.[0])).toBe(
            "https://idp.example/idp/oauth2/token",
        );
        const body = fetch.mock.calls[0]?.[1]?.body as URLSearchParams;
        expect(body.get("resource")).toBe("storage:filesystems");
        expect(JSON.parse(body.get("authorization_details") ?? "null")).toEqual(
            [{ type: "storage", actions: ["read", "write"] }],
        );
        expect(
            await Promise.all([
                socket.request({ type: "read", path: "secrets/secret.json" }),
                socket.request({
                    type: "write",
                    path: "secrets/secret.json",
                    content: [42],
                }),
            ]),
        ).toEqual([
            { type: "read", content: [42] },
            {
                type: "written",
                entry: {
                    name: "secret.json",
                    hash: "hash",
                    size: 1,
                    local: true,
                },
            },
        ]);
        expect(
            (socket.socket as unknown as TestSocket).send.mock.calls,
        ).toEqual([
            ['{"type":"read","path":"secrets/secret.json"}'],
            ['{"type":"write","path":"secrets/secret.json","content":[42]}'],
        ]);
        socket.close();
        expect(
            (socket.socket as unknown as TestSocket).close,
        ).toHaveBeenCalledOnce();
    });
    it("exchanges an access token before listing filesystems", async () => {
        const fetch = vi
            .fn()
            .mockResolvedValueOnce(
                Response.json({ access_token: "storage-token" }),
            )
            .mockResolvedValueOnce(
                Response.json([{ id: "filesystem-id", name: "Vault" }]),
            );
        const client = new StorageClient({
            idpBaseUrl: "https://idp.example/idp",
            storageBaseUrl: "https://storage.example/storage",
            clientId: "password-manager-web",
            bearerToken: () => "user-token",
            fetch,
        });

        await expect(client.listFileSystems()).resolves.toEqual([
            { id: "filesystem-id", name: "Vault" },
        ]);
        const exchange = fetch.mock.calls[0];
        expect(String(exchange?.[0])).toBe(
            "https://idp.example/idp/oauth2/token",
        );
        const body = exchange?.[1]?.body as URLSearchParams;
        expect(body.get("grant_type")).toBe(
            "urn:ietf:params:oauth:grant-type:token-exchange",
        );
        expect(body.get("subject_token")).toBe("user-token");
        expect(body.get("resource")).toBe("storage:filesystems");
        expect(body.has("audience")).toBe(false);
        expect(body.has("scope")).toBe(false);
        expect(body.get("subject_token_type")).toBe(
            "urn:ietf:params:oauth:token-type:access_token",
        );
        expect(String(fetch.mock.calls[1]?.[0])).toBe(
            "https://storage.example/storage/filesystems",
        );
        expect(JSON.parse(body.get("authorization_details") ?? "null")).toEqual(
            [{ type: "storage", actions: ["read"] }],
        );
        expect(fetch.mock.calls[1]?.[1]?.headers).toEqual({
            Authorization: "Bearer storage-token",
        });
    });
});

it("explicit SQL operations use database exchange and prefixed authorized HTTP without replay", async () => {
    const fetch = vi
        .fn()
        .mockResolvedValueOnce(
            Response.json({ access_token: "database-token" }),
        )
        .mockResolvedValueOnce(Response.json({ id: "db-one", name: "Label" }))
        .mockResolvedValueOnce(Response.json({ access_token: "read-token" }))
        .mockResolvedValueOnce(
            Response.json({
                results: [
                    {
                        columns: ["content"],
                        rows: [[{ type: "text", value: "record" }]],
                    },
                ],
            }),
        );
    const client = new StorageClient({
        idpBaseUrl: "https://idp.test/custom/idp",
        storageBaseUrl: "https://storage.test/custom/storage",
        clientId: "web",
        bearerToken: () => "user-token",
        fetch,
    });
    expect(fetch).not.toHaveBeenCalled();
    await expect(
        client.ensureDatabase({ creationKey: "chosen", name: "Label" }),
    ).resolves.toMatchObject({ id: "db-one" });
    await expect(
        client.executeSql(
            "db-one",
            {
                statements: [
                    { sql: "SELECT content FROM records", params: [] },
                ],
            },
            ["read"],
        ),
    ).resolves.toMatchObject({
        results: [{ rows: [[{ type: "text", value: "record" }]] }],
    });
    expect(fetch.mock.calls.map(([url]) => String(url))).toEqual([
        "https://idp.test/custom/idp/oauth2/token",
        "https://storage.test/custom/storage/databases/ensure",
        "https://idp.test/custom/idp/oauth2/token",
        "https://storage.test/custom/storage/databases/db-one/sql",
    ]);
    for (const index of [0, 2]) {
        const body = new URLSearchParams(fetch.mock.calls[index][1].body);
        expect(body.get("resource")).toBe("storage:databases");
        expect(body.get("subject_token")).toBe("user-token");
    }
    expect(
        JSON.parse(
            new URLSearchParams(fetch.mock.calls[2][1].body).get(
                "authorization_details",
            ) ?? "",
        ),
    ).toEqual([{ type: "storage", actions: ["read"] }]);
    expect(fetch.mock.calls[1][1].headers.Authorization).toBe(
        "Bearer database-token",
    );
    expect(fetch.mock.calls[3][1].headers.Authorization).toBe(
        "Bearer read-token",
    );
});

function databaseOperation(
    client: StorageClient,
    operation: string,
    signal?: AbortSignal,
) {
    switch (operation) {
        case "ensure":
            return client.ensureDatabase({ creationKey: "chosen" }, signal);
        case "create":
            return client.createDatabase("Label", signal);
        case "list":
            return client.listDatabases(signal);
        case "get":
            return client.getDatabase("db", signal);
        case "delete":
            return client.deleteDatabase("db", signal);
        default:
            return client.executeSql(
                "db",
                { statements: [{ sql: "SELECT id FROM records", params: [] }] },
                ["read"],
                signal,
            );
    }
}
it.each(["ensure", "create", "list", "get", "delete", "sql"])(
    "database %s cancellation while obtaining bearer cannot send exchange or operation",
    async (operation) => {
        let release!: (token: string) => void;
        const held = new Promise<string>((resolve) => {
            release = resolve;
        });
        const fetch = vi.fn();
        const client = new StorageClient({
            idpBaseUrl: "https://idp.test/idp",
            storageBaseUrl: "https://storage.test/storage",
            clientId: "web",
            bearerToken: () => held,
            fetch,
        });
        const controller = new AbortController();
        const pending = databaseOperation(client, operation, controller.signal);
        controller.abort();
        release("old-token");
        await expect(pending).rejects.toMatchObject({ name: "AbortError" });
        expect(fetch).not.toHaveBeenCalled();
    },
);
it.each(["ensure", "create", "list", "get", "delete", "sql"])(
    "database %s redacts failures and never automatically replays",
    async (operation) => {
        const fetch = vi
            .fn()
            .mockResolvedValueOnce(Response.json({ access_token: "db-token" }))
            .mockResolvedValueOnce(
                Response.json(
                    { code: "forbidden", details: "credential-bearing-body" },
                    { status: 403 },
                ),
            );
        const client = new StorageClient({
            idpBaseUrl: "https://idp.test/idp",
            storageBaseUrl: "https://storage.test/storage",
            clientId: "web",
            bearerToken: () => "user-token",
            fetch,
        });
        await expect(
            databaseOperation(client, operation),
        ).rejects.toMatchObject({
            name: "DatabaseOperationError",
            status: 403,
            message: "Database operation failed: 403",
        });
        expect(fetch).toHaveBeenCalledTimes(2);
        const body = new URLSearchParams(fetch.mock.calls[0][1].body);
        expect(body.get("resource")).toBe("storage:databases");
        expect(JSON.parse(body.get("authorization_details") ?? "")).toEqual([
            {
                type: "storage",
                actions: [
                    ["ensure", "create", "delete"].includes(operation)
                        ? "write"
                        : "read",
                ],
            },
        ]);
    },
);
