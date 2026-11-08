export * from './core';
export { createSync as createBrowserSync, BrowserSyncSDK } from './browser';
export { createSync as createNodeSync, NodeSyncSDK } from './node';
/** Create sync SDK instance for the current platform */
export declare function createSync(config: import('./core').SyncConfig): Promise<import('./core').SyncSDK>;
//# sourceMappingURL=index.d.ts.map