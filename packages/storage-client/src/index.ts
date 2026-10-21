import {
    Configuration as IdpConfiguration,
    DefaultApi as IdpApi,
    SubjectTokenType,
    ResponseError as IdpResponseError,
} from "@aicacia/idp-client";
import { ResourcesApi } from "./generated/apis/ResourcesApi.js";
import { DefaultApi as StorageApi } from "./generated/apis/DefaultApi.js";
import {
    Configuration,
    querystring,
    ResponseError,
} from "./generated/runtime.js";

export { ResourcesResidencyApi } from "./generated/apis/ResourcesResidencyApi.js";
export { ResidencyMode } from "./generated/models/ResidencyMode.js";
export type { ResourceResidency } from "./generated/models/ResourceResidency.js";
export { Configuration as StorageOwnerConfiguration } from "./generated/runtime.js";

export type StorageRequest =
    | { type: "read"; path: string }
    | { type: "write"; path: string; content: number[] }
    | { type: "append"; path: string; content: number[] }
    | { type: "delete"; path: string }
    | { type: "entry"; path: string }
    | { type: "list"; path: string };

export type StorageEntry = {
    name: string;
    hash: string;
    size: number;
    local: boolean;
};

export type StorageResponse =
    | { type: "authenticated" }
    | { type: "read"; content: number[] }
    | { type: "written"; entry: StorageEntry }
    | { type: "appended"; entry: StorageEntry }
    | { type: "deleted" }
    | { type: "entry"; entry: StorageEntry }
    | { type: "listed"; entries: StorageEntry[] }
    | { type: "error"; code: "invalidRequest" | "operationFailed" };

export type FileSystemResource = { id: string; name: string | null };

type FetchFunction = (
    input: URL | RequestInfo,
    init?: RequestInit,
) => Promise<Response>;

export type StorageClientOptions = {
    idpBaseUrl: URL | string;
    storageBaseUrl: URL | string;
    clientId: string;
    bearerToken: () => string | Promise<string>;
    fetch?: FetchFunction;
};

export class StorageSocket {
    #queue = Promise.resolve();

    constructor(readonly socket: WebSocket) {}

    request(request: StorageRequest): Promise<StorageResponse> {
        const response = this.#queue.then(async () => {
            const next = receive(this.socket);
            this.socket.send(JSON.stringify(request));
            return next;
        });
        this.#queue = response.then(
            () => undefined,
            () => undefined,
        );
        return response;
    }

    close(): void {
        this.socket.close();
    }
}

export class StorageClient {
    constructor(private readonly options: StorageClientOptions) {}

    async listFileSystems(): Promise<FileSystemResource[]> {
        const token = await this.exchangeToken(["read"]);
        return (
            await this.resources(token)
                .listFileSystems()
                .catch((error) => {
                    if (error instanceof ResponseError)
                        throw new Error(
                            `Filesystem list failed: ${error.response.status}`,
                        );
                    throw error;
                })
        ).map((resource) => ({ id: resource.id, name: resource.name ?? null }));
    }

    async createFileSystem(name?: string): Promise<FileSystemResource> {
        const token = await this.exchangeToken(["write"]);
        const resource = await this.resources(token)
            .createFileSystem({ createResourceRequest: { name: name ?? null } })
            .catch((error) => {
                if (error instanceof ResponseError)
                    throw new Error(
                        `Filesystem creation failed: ${error.response.status}`,
                    );
                throw error;
            });
        return { id: resource.id, name: resource.name ?? null };
    }

    async openSocket(filesystemId: string): Promise<StorageSocket> {
        if (!filesystemId) {
            throw new Error("Select a filesystem before opening storage");
        }
        const url = parseUrl(this.options.storageBaseUrl);
        const token = await this.exchangeToken(["read", "write"]);
        url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
        // The browser supplies handshake headers; the generated builder owns the URL/query.
        const operation = await new StorageApi(
            new Configuration(),
        ).upgradeSocketRequestOpts({
            accessToken: token,
            filesystemId,
            upgrade: "websocket",
            connection: "Upgrade",
            secWebSocketKey: "",
            secWebSocketVersion: "13",
        });
        url.pathname = url.pathname.replace(/\/+$/, "") + operation.path;
        url.search = querystring(operation.query ?? {});
        const socket = new WebSocket(url.href);
        await opened(socket);
        return new StorageSocket(socket);
    }

    private async exchangeToken(
        actions: ("read" | "write")[],
    ): Promise<string> {
        const subjectToken = await this.options.bearerToken();
        if (!subjectToken) {
            throw new Error("Missing OIDC access token");
        }
        const api = new IdpApi(
            new IdpConfiguration({
                basePath: parseUrl(this.options.idpBaseUrl).href.replace(
                    /\/$/,
                    "",
                ),
                fetchApi: this.options.fetch ?? fetch,
            }),
        );
        let result;
        try {
            result = await api.token({
                grantType: "urn:ietf:params:oauth:grant-type:token-exchange",
                subjectToken,
                subjectTokenType:
                    SubjectTokenType.UrnIetfParamsOauthTokenTypeAccessToken,
                resource: "storage:filesystems",
                clientId: this.options.clientId,
                authorizationDetails: JSON.stringify([
                    { type: "storage", actions },
                ]),
            });
        } catch (error) {
            if (error instanceof IdpResponseError)
                throw new Error(
                    `Storage token exchange failed: ${error.response.status}`,
                );
            throw error;
        }
        if (!result.accessToken) {
            throw new Error("Storage token exchange returned no access token");
        }
        return result.accessToken;
    }

    private resources(token: string): ResourcesApi {
        return new ResourcesApi(
            new Configuration({
                basePath: parseUrl(this.options.storageBaseUrl).href.replace(
                    /\/$/,
                    "",
                ),
                fetchApi: this.options.fetch ?? fetch,
                headers: { Authorization: `Bearer ${token}` },
            }),
        );
    }
}

function parseUrl(value: URL | string, path = ""): URL {
    const url = new URL(value);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
        throw new Error("Service base URL must use HTTP or HTTPS");
    }
    if (url.username || url.password || url.search || url.hash) {
        throw new Error(
            "Service base URL must not contain credentials, query or fragment",
        );
    }
    url.pathname = url.pathname.replace(/\/+$/, "") + path;
    return url;
}

function opened(socket: WebSocket): Promise<void> {
    if (socket.readyState === WebSocket.OPEN) {
        return Promise.resolve();
    }
    return new Promise((resolve, reject) => {
        const cleanup = () => {
            socket.removeEventListener("open", onOpen);
            socket.removeEventListener("error", onError);
        };
        const onOpen = () => {
            cleanup();
            resolve();
        };
        const onError = () => {
            cleanup();
            reject(new Error("Storage socket failed to open"));
        };
        socket.addEventListener("open", onOpen);
        socket.addEventListener("error", onError);
    });
}

function receive(socket: WebSocket): Promise<StorageResponse> {
    return new Promise((resolve, reject) => {
        const cleanup = () => {
            socket.removeEventListener("message", onMessage);
            socket.removeEventListener("close", onClose);
            socket.removeEventListener("error", onError);
        };
        const onMessage = (event: MessageEvent) => {
            cleanup();
            if (typeof event.data !== "string") {
                reject(
                    new Error("Storage socket returned a non-text response"),
                );
                return;
            }
            try {
                resolve(JSON.parse(event.data) as StorageResponse);
            } catch {
                reject(
                    new Error("Storage socket returned an invalid response"),
                );
            }
        };
        const onClose = () => {
            cleanup();
            reject(new Error("Storage socket closed"));
        };
        const onError = () => {
            cleanup();
            reject(new Error("Storage socket failed"));
        };
        socket.addEventListener("message", onMessage);
        socket.addEventListener("close", onClose);
        socket.addEventListener("error", onError);
    });
}
