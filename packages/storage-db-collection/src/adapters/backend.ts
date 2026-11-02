/** Minimal async storage backend contract for TanStack DB collections. */
export interface StorageBackend {
    /** Read a value by key. Returns undefined when the key is missing. */
    read(key: string): Promise<Uint8Array | undefined>;

    /** Write a value by key. Overwrites existing content. */
    write(key: string, bytes: Uint8Array): Promise<void>;

    /** Delete a value by key. */
    delete(key: string): Promise<void>;

    /** List all keys with the given prefix. Returns full keys. */
    list(prefix: string): Promise<string[]>;

    /** Optional cleanup hook. Called when the collection is disposed. */
    close?(): Promise<void> | void;

    /** Optional disconnect notification. Called when the backend detects disconnection. */
    onDisconnect?: (handler: () => void) => void;
}

/** Factory that connects to a backend, optionally respecting an AbortSignal. */
export type ConnectBackend = (signal?: AbortSignal) => Promise<StorageBackend>;

/** Result of a storage operation for acknowledgement tracking. */
export interface StorageAck {
    /** Whether the operation was explicitly acknowledged. */
    acknowledged: boolean;

    /** Whether the operation was explicitly rejected. */
    rejected: boolean;

    /** An error if the outcome is ambiguous (timeout, malformed response). */
    ambiguousError?: Error;
}
