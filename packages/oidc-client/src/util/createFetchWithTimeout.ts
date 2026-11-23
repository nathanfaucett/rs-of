export type Fetch = typeof fetch;

export function createFetchWithTimeout(timeoutMS: number, fetchFn: Fetch = fetch): Fetch {
  return async (input, init) => {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeoutMS);
    const signal = init?.signal
      ? AbortSignal.any([controller.signal, init.signal])
      : controller.signal;

    try {
      return await fetchFn(input, { ...init, signal });
    } finally {
      clearTimeout(timeoutId);
    }
  };
}
