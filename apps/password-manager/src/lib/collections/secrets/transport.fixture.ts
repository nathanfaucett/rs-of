import type { StorageRequest } from "@aicacia/storage-client";
export const id = "01900000-0000-7000-8000-000000000001";
export const secret = {
    id,
    name: "First secret",
    secret: "private",
    favorite: false,
    notes: "",
    uris: [
        { uri: "https://first.test", match: "exact" },
        { uri: "https://second.test", match: "host" },
    ],
    fields: [{ name: "Account", value: "kept", type: "hidden" }],
    history: [{ secret: "older", changedAt: "2025-01-01T00:00:00.000Z" }],
    deletedAt: "2025-02-01T00:00:00.000Z",
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
};
export const entry = (name: string) => ({
    name,
    hash: "h",
    size: 1,
    local: true,
});
export class Endpoint extends EventTarget {
    static OPEN = 1;
    static sockets: Endpoint[] = [];
    static files = new Map<string, Map<string, unknown>>();
    static rejectWrites = false;
    static ambiguousWrites = false;
    static holdWrites = false;
    readyState = 1;
    closed = false;
    closes = 0;
    requests: StorageRequest[] = [];
    hold = false;
    constructor(readonly url: string) {
        super();
        Endpoint.sockets.push(this);
    }
    send(data: string) {
        const request = JSON.parse(data) as StorageRequest;
        this.requests.push(request);
        if (this.hold) return;
        const fs = new URL(this.url).searchParams.get("filesystem_id");
        if (!fs) throw new Error("Test socket requires a Filesystem ID");
        let files = Endpoint.files.get(fs);
        if (!files) {
            files = new Map(
                fs === "empty" ? [] : [[`secrets/${id}.json`, secret]],
            );
            Endpoint.files.set(fs, files);
        }
        if (
            Endpoint.holdWrites &&
            (request.type === "write" || request.type === "delete")
        )
            return;
        if (!Endpoint.rejectWrites && request.type === "write")
            files.set(
                request.path,
                JSON.parse(
                    new TextDecoder().decode(new Uint8Array(request.content)),
                ),
            );
        if (!Endpoint.rejectWrites && request.type === "delete")
            files.delete(request.path);
        const response =
            request.type === "write" || request.type === "delete"
                ? Endpoint.rejectWrites
                    ? { type: "error", code: "operationFailed" }
                    : Endpoint.ambiguousWrites
                      ? { type: "authenticated" }
                      : request.type === "write"
                        ? { type: "written", entry: entry(request.path) }
                        : { type: "deleted" }
                : request.type === "list"
                  ? {
                        type: "listed",
                        entries:
                            request.path === ""
                                ? files.size === 0
                                    ? []
                                    : [entry("secrets")]
                                : [...files.keys()].map(entry),
                    }
                  : {
                        type: "read",
                        content: [
                            ...new TextEncoder().encode(
                                JSON.stringify(files.get(request.path)),
                            ),
                        ],
                    };
        queueMicrotask(() =>
            this.dispatchEvent(
                new MessageEvent("message", { data: JSON.stringify(response) }),
            ),
        );
    }
    close() {
        ++this.closes;
        this.closed = true;
        this.dispatchEvent(new Event("close"));
    }
}

export function required<T>(value: T | null | undefined): T {
    if (value == null) throw new Error("Expected a populated test fixture");
    return value;
}

export function resetTransport() {
    Endpoint.sockets = [];
    Endpoint.files = new Map();
    Endpoint.rejectWrites = false;
    Endpoint.ambiguousWrites = false;
    Endpoint.holdWrites = false;
}
