import {
    DatabaseOperationError,
    type SqlRequest,
    type SqlResponse,
} from "@aicacia/storage-client";

export type SqlStorage = {
    executeSql(
        id: string,
        request: SqlRequest,
        actions: ("read" | "write")[],
        signal?: AbortSignal,
    ): Promise<SqlResponse>;
};

export async function storageRequest<T>(
    operation: (signal: AbortSignal) => Promise<T>,
    signal: AbortSignal,
): Promise<T> {
    signal.throwIfAborted();
    const controller = new AbortController();
    const combined = AbortSignal.any([signal, controller.signal]);
    let timer: ReturnType<typeof setTimeout> | undefined;
    let abort: () => void = () => {};
    try {
        const cancelled = new Promise<never>((_, reject) => {
            abort = () =>
                reject(
                    new Error(
                        "Storage request interrupted; a sent write may have persisted.",
                    ),
                );
            combined.addEventListener("abort", abort, { once: true });
            timer = setTimeout(() => controller.abort(), 30_000);
        });
        const result = await Promise.race([operation(combined), cancelled]);
        combined.throwIfAborted();
        return result;
    } finally {
        clearTimeout(timer);
        combined.removeEventListener("abort", abort);
    }
}

export function storageMessage(cause: unknown): string {
    if (cause instanceof DatabaseOperationError) {
        switch (cause.status) {
            case 401:
            case 403:
                return "Storage access denied. Check authorization and retry explicitly.";
            case 404:
                return "Database unavailable or no longer present. No replacement was created.";
            case 410:
                return "Database deleted. Automatic recreation is disabled.";
            case 409:
                return "Database local copy is fenced. No replacement was created.";
            case 503:
                return "Database unavailable. No replacement was created.";
            case 413:
                return "Storage request or snapshot exceeds the supported size. No partial records were loaded.";
        }
    }
    return "Storage request failed. Check connectivity and authorization, then retry explicitly.";
}
