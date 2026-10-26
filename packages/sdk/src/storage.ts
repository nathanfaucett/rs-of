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
    constructor(connection: StorageSocket, active: () => void) {
        this.#connection = connection;
        this.#active = active;
        this.socket = Object.freeze({
            addEventListener: connection.socket.addEventListener.bind(
                connection.socket,
            ),
            removeEventListener: connection.socket.removeEventListener.bind(
                connection.socket,
            ),
        });
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
        this.#connection.close();
    }
}
