export function once<F extends (...args: never[]) => unknown>(fn: F): F {
    let called = false;
    let result: unknown;

    return ((...args: Parameters<F>): unknown => {
        if (!called) {
            called = true;
            result = fn(...args);
        }
        return result;
    }) as F;
}
