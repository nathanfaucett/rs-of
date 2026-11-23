// Main entry point for @ofsdk/sync-sdk
// Platform-specific implementations
export { BrowserSyncSDK, createSync as createBrowserSync } from "./browser";
export * from "./core";
export { createSync as createNodeSync, NodeSyncSDK } from "./node";
// Auto-detect platform and export appropriate createSync
import { createSync as createBrowserSync } from "./browser";
import { createSync as createNodeSync } from "./node";
/** Create sync SDK instance for the current platform */
export async function createSync(config) {
    // Detect platform
    if (typeof window !== "undefined" && typeof navigator !== "undefined") {
        return createBrowserSync(config);
    }
    if (typeof process !== "undefined" && process.versions?.node) {
        return createNodeSync(config);
    }
    throw new Error("Unsupported platform");
}
//# sourceMappingURL=index.js.map