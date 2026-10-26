import type {
    StorageConnection,
    StorageRequest,
    StorageResponse,
    StorageSocket,
} from "@aicacia/storage-client";

export class ContextSocket implements StorageConnection {
    readonly socket: StorageConnection["socket"];
    readonly #connection: StorageSocket;
    readonly #active: () => void;
    #pending = Promise.resolve();
    #closed = false;
    readonly #lifecycle = new AbortController();
    constructor(connection: StorageSocket, active: () => void) {
        this.#connection = connection;
        this.#active = active;
        const events = new EventTarget();
        this.socket = Object.freeze({
            addEventListener: events.addEventListener.bind(events),
            removeEventListener: events.removeEventListener.bind(events),
        });
        for (const type of ["close", "error"]) {
            connection.socket.addEventListener(
                type,
                () => {
                    events.dispatchEvent(new Event(type));
                    if (type === "close") this.#lifecycle.abort();
                },
                { signal: this.#lifecycle.signal },
            );
        }
    }
    #check() {
        this.#active();
        if (this.#closed) throw new Error("Storage connection is closed");
    }
    async request(value: StorageRequest): Promise<StorageResponse> {
        this.#check();
        const pending = this.#pending.then(async () => {
            this.#check();
            try {
                const response = await this.#connection.request(value);
                this.#check();
                return response;
            } catch (error) {
                this.#check();
                throw error;
            }
        });
        this.#pending = pending.then(
            () => undefined,
            () => undefined,
        );
        return pending;
    }
    close(): void {
        if (this.#closed) return;
        this.#closed = true;
        try {
            this.#connection.close();
        } finally {
            this.#lifecycle.abort();
        }
    }
}
