var __classPrivateFieldGet = (this && this.__classPrivateFieldGet) || function (receiver, state, kind, f) {
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a getter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot read private member from an object whose class did not declare it");
    return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
};
var __classPrivateFieldSet = (this && this.__classPrivateFieldSet) || function (receiver, state, value, kind, f) {
    if (kind === "m") throw new TypeError("Private method is not writable");
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a setter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot write private member to an object whose class did not declare it");
    return (kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value)), value;
};
var _StorageSocket_queue;
import { Configuration as IdpConfiguration, DefaultApi as IdpApi, SubjectTokenType, ResponseError as IdpResponseError, } from "@aicacia/idp-client";
import { ResourcesApi } from "./generated/apis/ResourcesApi.js";
import { DefaultApi as StorageApi } from "./generated/apis/DefaultApi.js";
import { Configuration, querystring, ResponseError, } from "./generated/runtime.js";
export { ResourcesResidencyApi } from "./generated/apis/ResourcesResidencyApi.js";
export { ResidencyMode } from "./generated/models/ResidencyMode.js";
export { Configuration as StorageOwnerConfiguration } from "./generated/runtime.js";
export class StorageSocket {
    constructor(socket) {
        this.socket = socket;
        _StorageSocket_queue.set(this, Promise.resolve());
    }
    request(request) {
        const response = __classPrivateFieldGet(this, _StorageSocket_queue, "f").then(async () => {
            const next = receive(this.socket);
            this.socket.send(JSON.stringify(request));
            return next;
        });
        __classPrivateFieldSet(this, _StorageSocket_queue, response.then(() => undefined, () => undefined), "f");
        return response;
    }
    close() {
        this.socket.close();
    }
}
_StorageSocket_queue = new WeakMap();
export class StorageClient {
    constructor(options) {
        this.options = options;
    }
    async listFileSystems() {
        const token = await this.exchangeToken(["read"]);
        return (await this.resources(token)
            .listFileSystems()
            .catch((error) => {
            if (error instanceof ResponseError)
                throw new Error(`Filesystem list failed: ${error.response.status}`);
            throw error;
        })).map((resource) => ({ id: resource.id, name: resource.name ?? null }));
    }
    async createFileSystem(name) {
        const token = await this.exchangeToken(["write"]);
        const resource = await this.resources(token)
            .createFileSystem({ createResourceRequest: { name: name ?? null } })
            .catch((error) => {
            if (error instanceof ResponseError)
                throw new Error(`Filesystem creation failed: ${error.response.status}`);
            throw error;
        });
        return { id: resource.id, name: resource.name ?? null };
    }
    async openSocket(filesystemId, signal) {
        signal?.throwIfAborted();
        if (!filesystemId) {
            throw new Error("Select a filesystem before opening storage");
        }
        const url = parseUrl(this.options.storageBaseUrl);
        const token = await this.exchangeToken(["read", "write"], signal);
        url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
        // The browser supplies handshake headers; the generated builder owns the URL/query.
        const operation = await new StorageApi(new Configuration()).upgradeSocketRequestOpts({
            accessToken: token,
            filesystemId,
            upgrade: "websocket",
            connection: "Upgrade",
            secWebSocketKey: "",
            secWebSocketVersion: "13",
        });
        url.pathname = url.pathname.replace(/\/+$/, "") + operation.path;
        url.search = querystring(operation.query ?? {});
        signal?.throwIfAborted();
        const socket = new WebSocket(url.href);
        try {
            await opened(socket, signal);
        }
        catch (error) {
            socket.close();
            throw error;
        }
        return new StorageSocket(socket);
    }
    async exchangeToken(actions, signal) {
        signal?.throwIfAborted();
        const subjectToken = await this.options.bearerToken();
        signal?.throwIfAborted();
        if (!subjectToken) {
            throw new Error("Missing OIDC access token");
        }
        const api = new IdpApi(new IdpConfiguration({
            basePath: parseUrl(this.options.idpBaseUrl).href.replace(/\/$/, ""),
            fetchApi: this.options.fetch ?? fetch,
        }));
        const result = await api
            .token({
            grantType: "urn:ietf:params:oauth:grant-type:token-exchange",
            subjectToken,
            subjectTokenType: SubjectTokenType.UrnIetfParamsOauthTokenTypeAccessToken,
            resource: "storage:filesystems",
            clientId: this.options.clientId,
            authorizationDetails: JSON.stringify([
                { type: "storage", actions },
            ]),
        }, signal ? { signal } : undefined)
            .catch((error) => {
            if (error instanceof IdpResponseError)
                throw new Error(`Storage token exchange failed: ${error.response.status}`);
            throw error;
        });
        if (!result.accessToken) {
            throw new Error("Storage token exchange returned no access token");
        }
        return result.accessToken;
    }
    resources(token) {
        return new ResourcesApi(new Configuration({
            basePath: parseUrl(this.options.storageBaseUrl).href.replace(/\/$/, ""),
            fetchApi: this.options.fetch ?? fetch,
            headers: { Authorization: `Bearer ${token}` },
        }));
    }
}
function parseUrl(value, path = "") {
    const url = new URL(value);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
        throw new Error("Service base URL must use HTTP or HTTPS");
    }
    if (url.username || url.password || url.search || url.hash) {
        throw new Error("Service base URL must not contain credentials, query or fragment");
    }
    url.pathname = url.pathname.replace(/\/+$/, "") + path;
    return url;
}
function opened(socket, signal) {
    if (socket.readyState === WebSocket.OPEN) {
        return Promise.resolve();
    }
    return new Promise((resolve, reject) => {
        const cleanup = () => {
            socket.removeEventListener("open", onOpen);
            socket.removeEventListener("error", onError);
            socket.removeEventListener("close", onError);
            signal?.removeEventListener("abort", onAbort);
        };
        const onAbort = () => {
            cleanup();
            reject(new Error("Storage socket opening cancelled"));
        };
        const onOpen = () => {
            cleanup();
            resolve();
        };
        const onError = () => {
            cleanup();
            reject(new Error("Storage socket failed to open"));
        };
        signal?.addEventListener("abort", onAbort, { once: true });
        socket.addEventListener("close", onError);
        socket.addEventListener("open", onOpen);
        socket.addEventListener("error", onError);
    });
}
function receive(socket) {
    return new Promise((resolve, reject) => {
        const cleanup = () => {
            socket.removeEventListener("message", onMessage);
            socket.removeEventListener("close", onClose);
            socket.removeEventListener("error", onError);
        };
        const onMessage = (event) => {
            cleanup();
            if (typeof event.data !== "string") {
                reject(new Error("Storage socket returned a non-text response"));
                return;
            }
            try {
                resolve(JSON.parse(event.data));
            }
            catch {
                reject(new Error("Storage socket returned an invalid response"));
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
//# sourceMappingURL=index.js.map