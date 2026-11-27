// Main entry point for @ofsdk/sync-sdk

// Platform-specific implementations
export {
  BrowserSyncSDK,
  createSqlResourceSync,
  createSync as createBrowserSync,
  type SqlResourceSync,
  type SqlResourceSyncOptions,
  type SqlResourceSyncStatus,
  SyncError,
  startSqlResourceSync,
} from "./browser";
export * from "./core";
export { createSync as createNodeSync, NodeSyncSDK } from "./node";

// Auto-detect platform and export appropriate createSync
import { createSync as createBrowserSync } from "./browser";
import { createSync as createNodeSync } from "./node";

/** Create sync SDK instance for the current platform */
export async function createSync(
  config: import("./core").SyncConfig
): Promise<import("./core").SyncSDK> {
  // Detect platform
  if (typeof window !== "undefined" && typeof navigator !== "undefined") {
    return createBrowserSync(config);
  }
  if (typeof process !== "undefined" && process.versions?.node) {
    return createNodeSync(config);
  }
  throw new Error("Unsupported platform");
}
