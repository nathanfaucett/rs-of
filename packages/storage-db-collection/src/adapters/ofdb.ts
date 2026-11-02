import type { ConnectBackend, StorageBackend } from "../adapters/backend.js";

/** Encodes a Uint8Array as a JSON array of numbers for KV storage. */
function encodeBytes(bytes: Uint8Array): number[] {
    return Array.from(bytes);
}

/** Decodes a JSON array of numbers back to Uint8Array. Validates byte range. */
function decodeBytes(value: unknown): Uint8Array {
    if (!Array.isArray(value)) {
        throw new Error("Stored value is not a byte array");
    }
    for (const byte of value) {
        if (
            typeof byte !== "number" ||
            !Number.isInteger(byte) ||
            byte < 0 ||
            byte > 255
        ) {
            throw new Error("Stored byte array contains invalid value");
        }
    }
    return new Uint8Array(value);
}

/** Minimal KvClient shape required by the ofdb backend. */
export interface OfdbKvClient {
    get<T = unknown>(key: string): Promise<T | undefined>;
    set(key: string, value: unknown): Promise<void>;
    delete(key: string): Promise<void>;
    scanPrefix(prefix: string): Promise<Array<{ key: string; value: unknown }>>;
}

/** Adapts a KvClient to the StorageBackend interface. */
export function createOfdbBackend(kvClient: OfdbKvClient): ConnectBackend {
    return async () => {
        const backend: StorageBackend = {
            async read(key: string): Promise<Uint8Array | undefined> {
                const value = await kvClient.get<unknown>(key);
                if (value === undefined) {
                    return undefined;
                }
                return decodeBytes(value);
            },

            async write(key: string, bytes: Uint8Array): Promise<void> {
                await kvClient.set(key, encodeBytes(bytes));
            },

            async delete(key: string): Promise<void> {
                await kvClient.delete(key);
            },

            async list(prefix: string): Promise<string[]> {
                const entries = await kvClient.scanPrefix(prefix);
                return entries.map((entry) => entry.key);
            },

            // Embedded backend - no close or disconnect handling needed
        };

        return backend;
    };
}
