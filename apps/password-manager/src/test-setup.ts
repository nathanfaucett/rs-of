// Test setup - runs in both jsdom and Playwright
// In Playwright (real browser), indexedDB is already available
// In jsdom, we need to polyfill it

if (typeof globalThis.indexedDB === "undefined") {
	// Only polyfill in jsdom environment
	const { indexedDB, IDBDatabase, IDBObjectStore, IDBTransaction, IDBRequest, IDBOpenDBRequest } = await import("fake-indexeddb");
	globalThis.indexedDB = indexedDB;
	globalThis.IDBDatabase = IDBDatabase;
	globalThis.IDBObjectStore = IDBObjectStore;
	globalThis.IDBTransaction = IDBTransaction;
	globalThis.IDBRequest = IDBRequest;
	globalThis.IDBOpenDBRequest = IDBOpenDBRequest;
	globalThis.DOMException = DOMException;
	console.log("[test-setup] jsdom test setup loaded with fake-indexeddb");
} else {
	console.log("[test-setup] Playwright test setup loaded (real IndexedDB)");
}

export {};
