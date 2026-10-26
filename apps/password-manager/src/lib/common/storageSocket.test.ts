import { afterEach, beforeEach, expect, it, vi } from "vitest";

const host = vi.hoisted(() => ({
    idpBaseUrl: "https://api.localhost:1355/idp",
    env: { PUBLIC_STORAGE_BASE_URL: "https://storage.localhost:1355/storage" },
    getAccessToken: vi.fn(),
}));
vi.mock("./state/sdk", () => ({
    getIdpApiUrl: () => host.idpBaseUrl,
    getSdk: () => ({ tokenProvider: () => host.getAccessToken }),
}));
vi.mock("$env/dynamic/public", () => ({ env: host.env }));
const fetchApi = vi.fn();

beforeEach(() => {
    host.idpBaseUrl = "https://api.localhost:1355/idp";
    host.env.PUBLIC_STORAGE_BASE_URL = "https://storage.localhost:1355/storage";
    host.getAccessToken.mockReset().mockResolvedValue("current-user-token");
    fetchApi
        .mockReset()
        .mockResolvedValueOnce(
            Response.json({ access_token: "filesystem-token" }),
        )
        .mockResolvedValueOnce(
            Response.json([{ id: "filesystem-id", name: "Vault" }]),
        );
    vi.stubGlobal("fetch", fetchApi);
});
afterEach(() => vi.unstubAllGlobals());

it("lists through the explicit Storage base and exchanges the current User at the configured IdP prefix", async () => {
    const { createStorageClient } = await import("./storageSocket");
    expect(await createStorageClient().listFileSystems()).toEqual([
        { id: "filesystem-id", name: "Vault" },
    ]);
    expect(
        fetchApi.mock.calls.map(([url]) => new URL(String(url)).href),
    ).toEqual([
        "https://api.localhost:1355/idp/oauth2/token",
        "https://storage.localhost:1355/storage/filesystems",
    ]);
    const body = fetchApi.mock.calls[0]?.[1]?.body as URLSearchParams;
    expect(Object.fromEntries(body)).toEqual({
        grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
        subject_token: "current-user-token",
        subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
        resource: "storage:filesystems",
        client_id: "password-manager-web",
        authorization_details: '[{"type":"storage","actions":["read"]}]',
    });
    expect(host.getAccessToken).toHaveBeenCalledOnce();
    expect(fetchApi.mock.calls[1]?.[1]?.headers).toEqual({
        Authorization: "Bearer filesystem-token",
    });
});

it("creates the Password Manager filesystem with write-only authorization", async () => {
    fetchApi
        .mockReset()
        .mockResolvedValueOnce(Response.json({ access_token: "write-token" }))
        .mockResolvedValueOnce(
            Response.json(
                { id: "filesystem-id", name: "Password Manager" },
                { status: 201 },
            ),
        );
    const { createStorageClient } = await import("./storageSocket");
    expect(
        await createStorageClient().createFileSystem("Password Manager"),
    ).toEqual({
        id: "filesystem-id",
        name: "Password Manager",
    });
    const body = fetchApi.mock.calls[0]?.[1]?.body as URLSearchParams;
    expect(body.get("resource")).toBe("storage:filesystems");
    expect(body.get("authorization_details")).toBe(
        '[{"type":"storage","actions":["write"]}]',
    );
    expect(fetchApi.mock.calls[1]?.[1]).toEqual({
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            Authorization: "Bearer write-token",
        },
        body: '{"name":"Password Manager"}',
    });
});

it("opens the selected filesystem on the explicit Storage socket base, not the IdP origin", async () => {
    class Socket {
        static OPEN = 1;
        readyState = 1;
        constructor(readonly url: string) {}
    }
    vi.stubGlobal("WebSocket", Socket);
    const { createStorageClient } = await import("./storageSocket");
    const socket = await createStorageClient().openSocket(
        "01900000-0000-7000-8000-000000000001",
    );
    expect((socket.socket as unknown as Socket).url).toBe(
        "wss://storage.localhost:1355/storage?access_token=filesystem-token&filesystem_id=01900000-0000-7000-8000-000000000001",
    );
    expect(socket.socket.readyState).toBe(1);
    const body = fetchApi.mock.calls[0]?.[1]?.body as URLSearchParams;
    expect(body.get("resource")).toBe("storage:filesystems");
    expect(body.get("authorization_details")).toBe(
        '[{"type":"storage","actions":["read","write"]}]',
    );
});

it.each(["idp", "storage"])(
    "fails closed without an explicit %s service base",
    async (service) => {
        if (service === "idp") host.idpBaseUrl = "";
        else host.env.PUBLIC_STORAGE_BASE_URL = "";
        const { createStorageClient } = await import("./storageSocket");
        expect(() => createStorageClient().listFileSystems()).toThrow(
            service === "idp"
                ? "IdP API URL is not configured"
                : "Storage API URL is not configured",
        );
        expect(fetchApi).not.toHaveBeenCalled();
        expect(host.getAccessToken).not.toHaveBeenCalled();
    },
);
