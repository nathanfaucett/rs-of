import {
  DefaultApi as IdpApi,
  Configuration as IdpConfiguration,
  ResponseError as IdpResponseError,
  SubjectTokenType,
} from "@ofsdk/idp-client";
import { DefaultApi as StorageApi } from "./generated/apis/DefaultApi.js";
import { ResourcesApi } from "./generated/apis/ResourcesApi.js";
import { ResourcesSqlApi } from "./generated/apis/ResourcesSqlApi.js";
import { SyncApi } from "./generated/apis/SyncApi.js";
import type {
  DatabaseDetailResponse,
  DatabaseResource,
  EnsureDatabaseRequest,
  SqlRequest,
  SqlResponse,
  SqlValue,
} from "./generated/models/index.js";
import type {
  SyncConnectRequest,
  SyncSessionResponse,
} from "./generated/models/index.js";
import {
  Configuration,
  querystring,
  ResponseError,
} from "./generated/runtime.js";

export { ResourcesResidencyApi } from "./generated/apis/ResourcesResidencyApi.js";
export type {
  DatabaseDetailResponse,
  DatabaseResource,
  EnsureDatabaseRequest,
  SqlRequest,
  SqlResponse,
  SqlStatement,
  SqlValue,
} from "./generated/models/index.js";
export { ResidencyMode } from "./generated/models/ResidencyMode.js";
export type { ResourceResidency } from "./generated/models/ResourceResidency.js";
export { Configuration as StorageOwnerConfiguration } from "./generated/runtime.js";
export type StorageAction = "read" | "write";

export class DatabaseOperationError extends Error {
  readonly status: number;
  constructor(status: number) {
    super(`Database operation failed: ${status}`);
    this.name = "DatabaseOperationError";
    this.status = status;
  }
}

async function databaseResult<T>(pending: Promise<T>): Promise<T> {
  try {
    return await pending;
  } catch (error) {
    if (error instanceof ResponseError)
      throw new DatabaseOperationError(error.response.status);
    throw error;
  }
}

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

/** SQL Database handle with built-in sync transport */
export class SqlDatabaseHandle {
  constructor(
    private readonly storageClient: StorageClient,
    private readonly databaseId: string,
    private readonly storeIdentity: { storeId: string; replicaId: string },
  ) {}

  /** Execute SQL query */
  async execute(sql: string, params?: unknown[]): Promise<any> {
    const result = await this.storageClient.executeSql(
      this.databaseId,
      {
        statements: [
          { sql, params: params ? (params as SqlValue[]) : undefined },
        ],
      },
      ["read", "write"],
    );
    return result;
  }

  /** Synchronize with remote peer */
  async synchronize(): Promise<void> {
    // TODO: Implement using SyncClient from @ofsdk/storage-sync-client
  }

  /** Get the database ID */
  get id(): string {
    return this.databaseId;
  }

  /** Get the store identity */
  get identity() {
    return this.storeIdentity;
  }
}

/** KV Store handle with built-in sync transport */
export class KvStoreHandle {
  constructor(
    private readonly storageClient: StorageClient,
    private readonly storeId: string,
    private readonly storeIdentity: { storeId: string; replicaId: string },
  ) {}

  /** Get the store ID */
  get id(): string {
    return this.storeId;
  }
}

/** Filesystem handle with built-in sync transport */
export class FilesystemHandle {
  constructor(
    private readonly storageClient: StorageClient,
    private readonly filesystemId: string,
    private readonly storeIdentity: { storeId: string; replicaId: string },
  ) {}

  /** Open a WebSocket connection for file operations */
  async openSocket(signal?: AbortSignal): Promise<StorageSocket> {
    return this.storageClient.openSocket(this.filesystemId, signal);
  }

  /** Get the filesystem ID */
  get id(): string {
    return this.filesystemId;
  }
}

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

export type StorageConnection = Readonly<{
  socket: Pick<EventTarget, "addEventListener" | "removeEventListener">;
  request(request: StorageRequest): Promise<StorageResponse>;
  close(): void;
}>;

export class StorageSocket {
  #queue = Promise.resolve();

  readonly socket: WebSocket;
  constructor(socket: WebSocket) {
    this.socket = socket;
  }

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
  static validateServiceBase(value: URL | string): void {
    parseUrl(value);
  }

  private readonly options: StorageClientOptions;
  constructor(options: StorageClientOptions) {
    this.options = options;
  }

  // SQL Database methods
  async createSqlDatabase(name?: string): Promise<SqlDatabaseHandle> {
    const token = await this.exchangeToken(
      ["write"],
      undefined,
      "storage:databases",
    );
    const resource = await this.resources(token).createDatabase(
      { createResourceRequest: { name: name ?? null } },
      undefined,
    );
    return new SqlDatabaseHandle(this, resource.id, {
      storeId: resource.id,
      replicaId: crypto.randomUUID(),
    });
  }

  async openSqlDatabase(databaseId: string): Promise<SqlDatabaseHandle> {
    const token = await this.exchangeToken(
      ["read"],
      undefined,
      "storage:databases",
    );
    const resource = await this.resources(token).getDatabase(
      { databaseId },
      undefined,
    );
    return new SqlDatabaseHandle(this, resource.id, {
      storeId: resource.id,
      replicaId: crypto.randomUUID(),
    });
  }

  async listSqlDatabases(signal?: AbortSignal): Promise<DatabaseResource[]> {
    const token = await this.exchangeToken(
      ["read"],
      signal,
      "storage:databases",
    );
    signal?.throwIfAborted();
    return databaseResult(this.resources(token).listDatabases({ signal }));
  }

  // KV Store methods
  async createKvStore(name?: string): Promise<KvStoreHandle> {
    // TODO: Implement when KV REST API is available
    return new KvStoreHandle(this, crypto.randomUUID(), {
      storeId: crypto.randomUUID(),
      replicaId: crypto.randomUUID(),
    });
  }

  async openKvStore(storeId: string): Promise<KvStoreHandle> {
    return new KvStoreHandle(this, storeId, {
      storeId,
      replicaId: crypto.randomUUID(),
    });
  }

  async listKvStores(
    signal?: AbortSignal,
  ): Promise<{ id: string; name: string | null }[]> {
    // TODO: Implement when KV REST API is available
    return [];
  }

  // Filesystem methods
  async createFilesystem(name?: string): Promise<FilesystemHandle> {
    const token = await this.exchangeToken(["write"], undefined);
    const resource = await this.resources(token).createFileSystem(
      { createResourceRequest: { name: name ?? null } },
      undefined,
    );
    return new FilesystemHandle(this, resource.id, {
      storeId: resource.id,
      replicaId: crypto.randomUUID(),
    });
  }

  async openFilesystem(filesystemId: string): Promise<FilesystemHandle> {
    const token = await this.exchangeToken(["read"], undefined);
    const resource = await this.resources(token).getFileSystem(
      { filesystemId },
      undefined,
    );
    return new FilesystemHandle(this, resource.id, {
      storeId: resource.id,
      replicaId: crypto.randomUUID(),
    });
  }

  // Sync methods
  async syncToDevice(
    resourceId: string,
    targetDeviceId: string,
  ): Promise<SyncSessionResponse> {
    const token = await this.exchangeToken(["write"], undefined);
    const api = new SyncApi(this.configuration(token));
    const request: SyncConnectRequest = {
      resourceId,
      targetDevice: targetDeviceId,
    };
    return databaseResult(
      api.syncConnect({ syncConnectRequest: request }, undefined),
    );
  }

  // Existing methods
  async ensureDatabase(
    request: EnsureDatabaseRequest,
    signal?: AbortSignal,
  ): Promise<DatabaseResource> {
    const token = await this.exchangeToken(
      ["write"],
      signal,
      "storage:databases",
    );
    signal?.throwIfAborted();
    return databaseResult(
      this.resources(token).ensureDatabase(
        { ensureDatabaseRequest: request },
        { signal },
      ),
    );
  }

  async createDatabase(
    name?: string,
    signal?: AbortSignal,
  ): Promise<DatabaseResource> {
    const token = await this.exchangeToken(
      ["write"],
      signal,
      "storage:databases",
    );
    signal?.throwIfAborted();
    return databaseResult(
      this.resources(token).createDatabase(
        { createResourceRequest: { name: name ?? null } },
        { signal },
      ),
    );
  }

  async listDatabases(signal?: AbortSignal): Promise<DatabaseResource[]> {
    const token = await this.exchangeToken(
      ["read"],
      signal,
      "storage:databases",
    );
    signal?.throwIfAborted();
    return databaseResult(this.resources(token).listDatabases({ signal }));
  }

  async getDatabase(
    databaseId: string,
    signal?: AbortSignal,
  ): Promise<DatabaseDetailResponse> {
    const token = await this.exchangeToken(
      ["read"],
      signal,
      "storage:databases",
    );
    signal?.throwIfAborted();
    return databaseResult(
      this.resources(token).getDatabase({ databaseId }, { signal }),
    );
  }

  async deleteDatabase(
    databaseId: string,
    signal?: AbortSignal,
  ): Promise<void> {
    const token = await this.exchangeToken(
      ["write"],
      signal,
      "storage:databases",
    );
    signal?.throwIfAborted();
    return databaseResult(
      this.resources(token).deleteDatabase({ databaseId }, { signal }),
    );
  }

  async executeSql(
    databaseId: string,
    request: SqlRequest,
    actions: StorageAction[],
    signal?: AbortSignal,
  ): Promise<SqlResponse> {
    const token = await this.exchangeToken(
      actions,
      signal,
      "storage:databases",
    );
    signal?.throwIfAborted();
    const api = new ResourcesSqlApi(this.configuration(token));
    return databaseResult(
      api.executeSql({ databaseId, sqlRequest: request }, { signal }),
    );
  }

  async listFileSystems(signal?: AbortSignal): Promise<FileSystemResource[]> {
    const token = await this.exchangeToken(["read"], signal);
    signal?.throwIfAborted();
    return (
      await this.resources(token)
        .listFileSystems(signal ? { signal } : undefined)
        .catch((error) => {
          if (error instanceof ResponseError)
            throw new Error(`Filesystem list failed: ${error.response.status}`);
          throw error;
        })
    ).map((resource) => ({ id: resource.id, name: resource.name ?? null }));
  }

  async createFileSystem(
    name?: string,
    signal?: AbortSignal,
  ): Promise<FileSystemResource> {
    const token = await this.exchangeToken(["write"], signal);
    signal?.throwIfAborted();
    const resource = await this.resources(token)
      .createFileSystem(
        { createResourceRequest: { name: name ?? null } },
        signal ? { signal } : undefined,
      )
      .catch((error) => {
        if (error instanceof ResponseError)
          throw new Error(
            `Filesystem creation failed: ${error.response.status}`,
          );
        throw error;
      });
    return { id: resource.id, name: resource.name ?? null };
  }

  async openSocket(
    filesystemId: string,
    signal?: AbortSignal,
  ): Promise<StorageSocket> {
    signal?.throwIfAborted();
    if (!filesystemId) {
      throw new Error("Select a filesystem before opening storage");
    }
    const url = parseUrl(this.options.storageBaseUrl);
    const token = await this.exchangeToken(["read", "write"], signal);
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
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
    signal?.throwIfAborted();
    const socket = new WebSocket(url.href);
    try {
      await opened(socket, signal);
    } catch (error) {
      socket.close();
      throw error;
    }
    return new StorageSocket(socket);
  }

  /** Get a Storage capability token for a resource and actions */
  async getStorageToken(
    resource: string,
    actions: ("read" | "write")[],
    signal?: AbortSignal,
  ): Promise<string> {
    return this.exchangeToken(actions, signal, resource);
  }

  private async exchangeToken(
    actions: ("read" | "write")[],
    signal?: AbortSignal,
    resource = "storage:filesystems",
  ): Promise<string> {
    signal?.throwIfAborted();
    const subjectToken = await this.options.bearerToken();
    signal?.throwIfAborted();
    if (!subjectToken) {
      throw new Error("Missing OIDC access token");
    }
    const api = new IdpApi(
      new IdpConfiguration({
        basePath: parseUrl(this.options.idpBaseUrl).href.replace(/\/$/, ""),
        fetchApi: this.options.fetch ?? fetch,
      }),
    );
    const result = await api
      .token(
        {
          grantType: "urn:ietf:params:oauth:grant-type:token-exchange",
          subjectToken,
          subjectTokenType:
            SubjectTokenType.UrnIetfParamsOauthTokenTypeAccessToken,
          resource,
          clientId: this.options.clientId,
          authorizationDetails: JSON.stringify([{ type: "storage", actions }]),
        },
        signal ? { signal } : undefined,
      )
      .catch((error: unknown) => {
        if (error instanceof IdpResponseError)
          throw new Error(
            `Storage token exchange failed: ${error.response.status}`,
          );
        throw error;
      });
    if (!result.accessToken) {
      throw new Error("Storage token exchange returned no access token");
    }
    return result.accessToken;
  }

  private resources(token: string): ResourcesApi {
    return new ResourcesApi(this.configuration(token));
  }

  private configuration(token: string): Configuration {
    return new Configuration({
      basePath: parseUrl(this.options.storageBaseUrl).href.replace(/\/$/, ""),
      fetchApi: this.options.fetch ?? fetch,
      headers: { Authorization: `Bearer ${token}` },
    });
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

function opened(socket: WebSocket, signal?: AbortSignal): Promise<void> {
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
      reject(new Error("Storage socket opening failed"));
    };
    socket.addEventListener("open", onOpen, { once: true });
    socket.addEventListener("error", onError, { once: true });
    socket.addEventListener("close", onError, { once: true });
    signal?.addEventListener("abort", onAbort, { once: true });
  });
}

function receive(socket: WebSocket): Promise<StorageResponse> {
  if (socket.readyState !== WebSocket.OPEN) {
    return Promise.reject(new Error("Storage socket not open"));
  }
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      socket.removeEventListener("message", onMessage);
      socket.removeEventListener("error", onError);
      socket.removeEventListener("close", onError);
      signal?.removeEventListener("abort", onAbort);
    };
    const onAbort = () => {
      cleanup();
      reject(new Error("Storage request cancelled"));
    };
    const onMessage = (event: MessageEvent) => {
      cleanup();
      try {
        const response = JSON.parse(event.data) as StorageResponse;
        resolve(response);
      } catch {
        reject(new Error("Invalid storage response"));
      }
    };
    const onError = () => {
      cleanup();
      reject(new Error("Storage socket error"));
    };
    socket.addEventListener("message", onMessage, { once: true });
    socket.addEventListener("error", onError, { once: true });
    socket.addEventListener("close", onError, { once: true });
    const signal = new AbortController().signal;
    signal.addEventListener("abort", onAbort, { once: true });
  });
}
