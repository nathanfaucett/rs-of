/* @ts-self-types="./ofdb_wasm.d.ts" */
import { commit, ensureOpen, open, snapshot } from './snippets/ofdb-btree-indexeddb-8bc291a7517dcebb/src/indexeddb.js';
import * as import1 from "./snippets/ofdb-btree-indexeddb-8bc291a7517dcebb/src/indexeddb.js"


/**
 * Opaque change work handle.
 */
export class JsKvChangeWork {
    static __wrap(ptr) {
        const obj = Object.create(JsKvChangeWork.prototype);
        obj.__wbg_ptr = ptr;
        JsKvChangeWorkFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        JsKvChangeWorkFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_jskvchangework_free(ptr, 0);
    }
}
if (Symbol.dispose) JsKvChangeWork.prototype[Symbol.dispose] = JsKvChangeWork.prototype.free;

/**
 * Sync error.
 */
export class JsKvSyncError {
    static __wrap(ptr) {
        const obj = Object.create(JsKvSyncError.prototype);
        obj.__wbg_ptr = ptr;
        JsKvSyncErrorFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        JsKvSyncErrorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_jskvsyncerror_free(ptr, 0);
    }
    /**
     * @returns {JsKvSyncErrorKind}
     */
    get kind() {
        const ret = wasm.jskvsyncerror_kind(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {string}
     */
    get message() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.jskvsyncerror_message(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) JsKvSyncError.prototype[Symbol.dispose] = JsKvSyncError.prototype.free;

/**
 * Sync error kind.
 * @enum {0 | 1 | 2 | 3 | 4 | 5 | 6 | 7}
 */
export const JsKvSyncErrorKind = Object.freeze({
    PreApply: 0, "0": "PreApply",
    Transport: 1, "1": "Transport",
    Codec: 2, "2": "Codec",
    InvalidFrame: 3, "3": "InvalidFrame",
    FrameTooLarge: 4, "4": "FrameTooLarge",
    UnsupportedVersion: 5, "5": "UnsupportedVersion",
    UnexpectedMessage: 6, "6": "UnexpectedMessage",
    Storage: 7, "7": "Storage",
});

/**
 * Sync transport trait that can be implemented in JavaScript.
 */
export class JsKvSyncTransport {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        JsKvSyncTransportFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_jskvsynctransport_free(ptr, 0);
    }
    /**
     * @param {any} transport
     */
    constructor(transport) {
        const ret = wasm.jskvsynctransport_new(transport);
        this.__wbg_ptr = ret;
        JsKvSyncTransportFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) JsKvSyncTransport.prototype[Symbol.dispose] = JsKvSyncTransport.prototype.free;

/**
 * Transport error for JS-implemented transports.
 */
export class JsKvSyncTransportError {
    static __wrap(ptr) {
        const obj = Object.create(JsKvSyncTransportError.prototype);
        obj.__wbg_ptr = ptr;
        JsKvSyncTransportErrorFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        JsKvSyncTransportErrorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_jskvsynctransporterror_free(ptr, 0);
    }
    /**
     * @returns {string}
     */
    get message() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.jskvsynctransporterror_message(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} message
     */
    constructor(message) {
        const ptr0 = passStringToWasm0(message, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.jskvsynctransporterror_new(ptr0, len0);
        this.__wbg_ptr = ret;
        JsKvSyncTransportErrorFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) JsKvSyncTransportError.prototype[Symbol.dispose] = JsKvSyncTransportError.prototype.free;

/**
 * Sync error.
 */
export class JsSqlSyncError {
    static __wrap(ptr) {
        const obj = Object.create(JsSqlSyncError.prototype);
        obj.__wbg_ptr = ptr;
        JsSqlSyncErrorFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        JsSqlSyncErrorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_jssqlsyncerror_free(ptr, 0);
    }
    /**
     * @returns {JsSqlSyncErrorKind}
     */
    get kind() {
        const ret = wasm.jssqlsyncerror_kind(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {string}
     */
    get message() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.jssqlsyncerror_message(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) JsSqlSyncError.prototype[Symbol.dispose] = JsSqlSyncError.prototype.free;

/**
 * Sync error kind.
 * @enum {0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10}
 */
export const JsSqlSyncErrorKind = Object.freeze({
    PreApply: 0, "0": "PreApply",
    Engine: 1, "1": "Engine",
    Transport: 2, "2": "Transport",
    Staging: 3, "3": "Staging",
    InvalidConfiguration: 4, "4": "InvalidConfiguration",
    Protocol: 5, "5": "Protocol",
    MessageTooLarge: 6, "6": "MessageTooLarge",
    SessionTooLarge: 7, "7": "SessionTooLarge",
    IncompatibleProtocol: 8, "8": "IncompatibleProtocol",
    UnexpectedMessage: 9, "9": "UnexpectedMessage",
    RemoteAbort: 10, "10": "RemoteAbort",
});

/**
 * Sync transport trait that can be implemented in JavaScript.
 */
export class JsSqlSyncTransport {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        JsSqlSyncTransportFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_jssqlsynctransport_free(ptr, 0);
    }
    /**
     * @param {any} transport
     */
    constructor(transport) {
        const ret = wasm.jssqlsynctransport_new(transport);
        this.__wbg_ptr = ret;
        JsSqlSyncTransportFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) JsSqlSyncTransport.prototype[Symbol.dispose] = JsSqlSyncTransport.prototype.free;

/**
 * Transport error for JS-implemented transports.
 */
export class JsSqlSyncTransportError {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        JsSqlSyncTransportErrorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_jssqlsynctransporterror_free(ptr, 0);
    }
    /**
     * @returns {string}
     */
    get message() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.jssqlsynctransporterror_message(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} message
     */
    constructor(message) {
        const ptr0 = passStringToWasm0(message, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.jssqlsynctransporterror_new(ptr0, len0);
        this.__wbg_ptr = ret;
        JsSqlSyncTransportErrorFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) JsSqlSyncTransportError.prototype[Symbol.dispose] = JsSqlSyncTransportError.prototype.free;

/**
 * Change subscription for continuous synchronization.
 */
export class KvChangeSubscription {
    static __wrap(ptr) {
        const obj = Object.create(KvChangeSubscription.prototype);
        obj.__wbg_ptr = ptr;
        KvChangeSubscriptionFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        KvChangeSubscriptionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_kvchangesubscription_free(ptr, 0);
    }
    /**
     * Acknowledge completed work.
     * @param {JsKvChangeWork} work
     * @returns {boolean}
     */
    acknowledge(work) {
        _assertClass(work, JsKvChangeWork);
        var ptr0 = work.__destroy_into_raw();
        const ret = wasm.kvchangesubscription_acknowledge(this.__wbg_ptr, ptr0);
        return ret !== 0;
    }
    /**
     * Wait for changes.
     * @returns {Promise<void>}
     */
    changed() {
        const ret = wasm.kvchangesubscription_changed(this.__wbg_ptr);
        return ret;
    }
    /**
     * Merge with another subscription.
     * @param {KvChangeSubscription} other
     */
    merge(other) {
        _assertClass(other, KvChangeSubscription);
        var ptr0 = other.__destroy_into_raw();
        wasm.kvchangesubscription_merge(this.__wbg_ptr, ptr0);
    }
    /**
     * Select the next unit of work to synchronize.
     * @returns {JsKvChangeWork | undefined}
     */
    select() {
        const ret = wasm.kvchangesubscription_select(this.__wbg_ptr);
        return ret === 0 ? undefined : JsKvChangeWork.__wrap(ret);
    }
}
if (Symbol.dispose) KvChangeSubscription.prototype[Symbol.dispose] = KvChangeSubscription.prototype.free;

export class KvClient {
    static __wrap(ptr) {
        const obj = Object.create(KvClient.prototype);
        obj.__wbg_ptr = ptr;
        KvClientFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        KvClientFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_kvclient_free(ptr, 0);
    }
    /**
     * Return an independent clone sharing the same storage.
     * @returns {KvClient}
     */
    clone_client() {
        const ret = wasm.kvclient_clone_client(this.__wbg_ptr);
        return KvClient.__wrap(ret);
    }
    /**
     * Remove a committed value.
     * @param {string} key
     * @returns {Promise<void>}
     */
    delete(key) {
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.kvclient_delete(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Read a committed value as JSON; a missing key becomes JavaScript undefined.
     * @param {string} key
     * @returns {Promise<string | undefined>}
     */
    get(key) {
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.kvclient_get(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Scan entries with keys in [start, end) as a JSON array of {key, value} objects.
     * @param {string} start
     * @param {string} end
     * @returns {Promise<string>}
     */
    scan(start, end) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(start, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(end, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.kvclient_scan(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Scan all entries.
     * @returns {Promise<string>}
     */
    scan_all() {
        const ret = wasm.kvclient_scan_all(this.__wbg_ptr);
        return ret;
    }
    /**
     * Scan entries whose keys start with `prefix`.
     * @param {string} prefix
     * @returns {Promise<string>}
     */
    scan_prefix(prefix) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.kvclient_scan_prefix(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Store a JSON value as the committed value, with optional expiry (Unix epoch millis).
     * @param {string} key
     * @param {string} value
     * @param {bigint | null} [expires_at]
     * @returns {Promise<void>}
     */
    set(key, value, expires_at) {
        const val3 = BigInt.asIntN(64, (isLikeNone(expires_at) ? BigInt(0) : expires_at));
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(value, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.kvclient_set(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(expires_at), val3);
        return ret;
    }
    /**
     * Start a transaction that holds operations until commit or rollback.
     * @returns {Promise<KvTransaction>}
     */
    transaction() {
        const ret = wasm.kvclient_transaction(this.__wbg_ptr);
        return ret;
    }
    /**
     * Return a clone of this client with a request deadline in milliseconds.
     * @param {bigint} ms
     * @returns {KvClient}
     */
    with_deadline(ms) {
        const ret = wasm.kvclient_with_deadline(this.__wbg_ptr, ms);
        return KvClient.__wrap(ret);
    }
}
if (Symbol.dispose) KvClient.prototype[Symbol.dispose] = KvClient.prototype.free;

export class KvDatabase {
    static __wrap(ptr) {
        const obj = Object.create(KvDatabase.prototype);
        obj.__wbg_ptr = ptr;
        KvDatabaseFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        KvDatabaseFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_kvdatabase_free(ptr, 0);
    }
    /**
     * Create a query handle sharing this database's storage.
     * @returns {KvClient}
     */
    client() {
        const ret = wasm.kvdatabase_client(this.__wbg_ptr);
        return KvClient.__wrap(ret);
    }
    /**
     * Open a key-value database backed by IndexedDB.
     * @param {string} name
     * @returns {Promise<KvDatabase>}
     */
    static open(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.kvdatabase_open(ptr0, len0);
        return ret;
    }
    /**
     * Get the change subscription for continuous synchronization.
     * @returns {KvChangeSubscription}
     */
    sync_changes() {
        const ret = wasm.kvdatabase_sync_changes(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return KvChangeSubscription.__wrap(ret[0]);
    }
    /**
     * Synchronize with a remote peer using the given transport.
     * @param {JsKvSyncTransport} transport
     * @param {KvSyncRole} role
     * @param {KvSyncConfig} config
     * @returns {Promise<void>}
     */
    synchronize(transport, role, config) {
        _assertClass(transport, JsKvSyncTransport);
        const val2 = +role;
        _assertClass(config, KvSyncConfig);
        var ptr0 = config.__destroy_into_raw();
        const ret = wasm.kvdatabase_synchronize(this.__wbg_ptr, transport.__wbg_ptr, val2, ptr0);
        return ret;
    }
    /**
     * Synchronize with a remote peer, checking caller authorization before each batch.
     * @param {JsKvSyncTransport} transport
     * @param {KvSyncRole} role
     * @param {KvSyncConfig} config
     * @param {Function} check
     * @returns {Promise<void>}
     */
    synchronize_checked(transport, role, config, check) {
        _assertClass(transport, JsKvSyncTransport);
        const val2 = +role;
        _assertClass(config, KvSyncConfig);
        var ptr0 = config.__destroy_into_raw();
        const ret = wasm.kvdatabase_synchronize_checked(this.__wbg_ptr, transport.__wbg_ptr, val2, ptr0, check);
        return ret;
    }
    /**
     * Synchronize selected changes with a remote peer, checking caller authorization.
     * @param {KvChangeSubscription} subscription
     * @param {JsKvSyncTransport} transport
     * @param {KvSyncRole} role
     * @param {KvSyncConfig} config
     * @param {Function} check
     * @returns {Promise<void>}
     */
    synchronize_selected_checked(subscription, transport, role, config, check) {
        _assertClass(subscription, KvChangeSubscription);
        _assertClass(transport, JsKvSyncTransport);
        const val2 = +role;
        _assertClass(config, KvSyncConfig);
        var ptr0 = config.__destroy_into_raw();
        const ret = wasm.kvdatabase_synchronize_selected_checked(this.__wbg_ptr, subscription.__wbg_ptr, transport.__wbg_ptr, val2, ptr0, check);
        return ret;
    }
}
if (Symbol.dispose) KvDatabase.prototype[Symbol.dispose] = KvDatabase.prototype.free;

/**
 * Sync configuration.
 */
export class KvSyncConfig {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        KvSyncConfigFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_kvsyncconfig_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    get max_frame_bytes() {
        const ret = wasm.kvsyncconfig_max_frame_bytes(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get max_inventory_bytes() {
        const ret = wasm.kvsyncconfig_max_inventory_bytes(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get max_inventory_entries() {
        const ret = wasm.kvsyncconfig_max_inventory_entries(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get max_snapshots_per_batch() {
        const ret = wasm.kvsyncconfig_max_snapshots_per_batch(this.__wbg_ptr);
        return ret >>> 0;
    }
    constructor() {
        const ret = wasm.kvsyncconfig_new();
        this.__wbg_ptr = ret;
        KvSyncConfigFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {number} value
     */
    set max_frame_bytes(value) {
        wasm.kvsyncconfig_set_max_frame_bytes(this.__wbg_ptr, value);
    }
    /**
     * @param {number} value
     */
    set max_inventory_bytes(value) {
        wasm.kvsyncconfig_set_max_inventory_bytes(this.__wbg_ptr, value);
    }
    /**
     * @param {number} value
     */
    set max_inventory_entries(value) {
        wasm.kvsyncconfig_set_max_inventory_entries(this.__wbg_ptr, value);
    }
    /**
     * @param {number} value
     */
    set max_snapshots_per_batch(value) {
        wasm.kvsyncconfig_set_max_snapshots_per_batch(this.__wbg_ptr, value);
    }
}
if (Symbol.dispose) KvSyncConfig.prototype[Symbol.dispose] = KvSyncConfig.prototype.free;

/**
 * Sync role: Initiator or Responder.
 * @enum {0 | 1}
 */
export const KvSyncRole = Object.freeze({
    Initiator: 0, "0": "Initiator",
    Responder: 1, "1": "Responder",
});

export class KvTransaction {
    static __wrap(ptr) {
        const obj = Object.create(KvTransaction.prototype);
        obj.__wbg_ptr = ptr;
        KvTransactionFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        KvTransactionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_kvtransaction_free(ptr, 0);
    }
    /**
     * @returns {Promise<void>}
     */
    commit() {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.kvtransaction_commit(ptr);
        return ret;
    }
    /**
     * @param {string} key
     * @returns {Promise<void>}
     */
    delete(key) {
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.kvtransaction_delete(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} key
     * @returns {Promise<string | undefined>}
     */
    get(key) {
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.kvtransaction_get(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {Promise<void>}
     */
    rollback() {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.kvtransaction_rollback(ptr);
        return ret;
    }
    /**
     * @param {string} start
     * @param {string} end
     * @returns {Promise<string>}
     */
    scan(start, end) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(start, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(end, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.kvtransaction_scan(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @returns {Promise<string>}
     */
    scan_all() {
        const ret = wasm.kvtransaction_scan_all(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} prefix
     * @returns {Promise<string>}
     */
    scan_prefix(prefix) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.kvtransaction_scan_prefix(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} key
     * @param {string} value
     * @param {bigint | null} [expires_at]
     * @returns {Promise<void>}
     */
    set(key, value, expires_at) {
        const val3 = BigInt.asIntN(64, (isLikeNone(expires_at) ? BigInt(0) : expires_at));
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(value, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.kvtransaction_set(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(expires_at), val3);
        return ret;
    }
}
if (Symbol.dispose) KvTransaction.prototype[Symbol.dispose] = KvTransaction.prototype.free;

export class SqlClient {
    static __wrap(ptr) {
        const obj = Object.create(SqlClient.prototype);
        obj.__wbg_ptr = ptr;
        SqlClientFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SqlClientFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_sqlclient_free(ptr, 0);
    }
    /**
     * Return an independent clone sharing the same storage.
     * @returns {SqlClient}
     */
    clone_client() {
        const ret = wasm.sqlclient_clone_client(this.__wbg_ptr);
        return SqlClient.__wrap(ret);
    }
    /**
     * Execute SQL text with optional bound parameters and return results as JSON.
     *
     * Parameters are a JSON array for positional placeholders or a JSON object for
     * named placeholders. Every value uses the owner's lossless tagged codec.
     * The result is a JSON array of `{ columns, rows }` objects with tagged cells.
     * @param {string} sql
     * @param {string | null} [params]
     * @returns {Promise<string>}
     */
    execute(sql, params) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(sql, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = isLikeNone(params) ? 0 : passStringToWasm0(params, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.sqlclient_execute(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Apply the complete application migration history through the Rust runner.
     * @param {string} migrations
     * @returns {Promise<string>}
     */
    migrate(migrations) {
        const ptr0 = passStringToWasm0(migrations, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sqlclient_migrate(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Start a transaction that holds statements until commit or rollback.
     * @returns {Promise<SqlTransaction>}
     */
    transaction() {
        const ret = wasm.sqlclient_transaction(this.__wbg_ptr);
        return ret;
    }
    /**
     * Return a clone of this client with a request deadline in milliseconds.
     * @param {bigint} ms
     * @returns {SqlClient}
     */
    with_deadline(ms) {
        const ret = wasm.sqlclient_with_deadline(this.__wbg_ptr, ms);
        return SqlClient.__wrap(ret);
    }
}
if (Symbol.dispose) SqlClient.prototype[Symbol.dispose] = SqlClient.prototype.free;

export class SqlDatabase {
    static __wrap(ptr) {
        const obj = Object.create(SqlDatabase.prototype);
        obj.__wbg_ptr = ptr;
        SqlDatabaseFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SqlDatabaseFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_sqldatabase_free(ptr, 0);
    }
    /**
     * Create a query handle sharing this database's storage.
     * @returns {SqlClient}
     */
    client() {
        const ret = wasm.sqldatabase_client(this.__wbg_ptr);
        return SqlClient.__wrap(ret);
    }
    /**
     * Open a SQL database backed by IndexedDB.
     * @param {string} name
     * @returns {Promise<SqlDatabase>}
     */
    static open(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sqldatabase_open(ptr0, len0);
        return ret;
    }
    /**
     * Synchronize with a remote peer using the given transport.
     * @param {JsSqlSyncTransport} transport
     * @param {SqlSessionConfig} config
     * @param {SqlSyncRole} role
     * @returns {Promise<SqlSyncResult>}
     */
    synchronize(transport, config, role) {
        _assertClass(transport, JsSqlSyncTransport);
        const val2 = +role;
        _assertClass(config, SqlSessionConfig);
        var ptr0 = config.__destroy_into_raw();
        const ret = wasm.sqldatabase_synchronize(this.__wbg_ptr, transport.__wbg_ptr, ptr0, val2);
        return ret;
    }
    /**
     * Synchronize with a remote peer, checking caller authorization before each batch.
     * @param {JsSqlSyncTransport} transport
     * @param {SqlSessionConfig} config
     * @param {SqlSyncRole} role
     * @param {Function} check
     * @returns {Promise<SqlSyncResult>}
     */
    synchronize_checked(transport, config, role, check) {
        _assertClass(transport, JsSqlSyncTransport);
        const val2 = +role;
        _assertClass(config, SqlSessionConfig);
        var ptr0 = config.__destroy_into_raw();
        const ret = wasm.sqldatabase_synchronize_checked(this.__wbg_ptr, transport.__wbg_ptr, ptr0, val2, check);
        return ret;
    }
}
if (Symbol.dispose) SqlDatabase.prototype[Symbol.dispose] = SqlDatabase.prototype.free;

/**
 * Sync configuration.
 */
export class SqlSessionConfig {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SqlSessionConfigFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_sqlsessionconfig_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    get max_session_bytes() {
        const ret = wasm.sqlsessionconfig_max_session_bytes(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get max_units_per_frame() {
        const ret = wasm.sqlsessionconfig_max_units_per_frame(this.__wbg_ptr);
        return ret >>> 0;
    }
    constructor() {
        const ret = wasm.sqlsessionconfig_new();
        this.__wbg_ptr = ret;
        SqlSessionConfigFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {number} value
     */
    set max_session_bytes(value) {
        wasm.sqlsessionconfig_set_max_session_bytes(this.__wbg_ptr, value);
    }
    /**
     * @param {number} value
     */
    set max_units_per_frame(value) {
        wasm.sqlsessionconfig_set_max_units_per_frame(this.__wbg_ptr, value);
    }
}
if (Symbol.dispose) SqlSessionConfig.prototype[Symbol.dispose] = SqlSessionConfig.prototype.free;

/**
 * Sync direction.
 * @enum {0}
 */
export const SqlSyncDirection = Object.freeze({
    Bidirectional: 0, "0": "Bidirectional",
});

/**
 * Sync result.
 */
export class SqlSyncResult {
    static __wrap(ptr) {
        const obj = Object.create(SqlSyncResult.prototype);
        obj.__wbg_ptr = ptr;
        SqlSyncResultFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SqlSyncResultFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_sqlsyncresult_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    get received_changes() {
        const ret = wasm.sqlsyncresult_received_changes(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get received_snapshots() {
        const ret = wasm.sqlsyncresult_received_snapshots(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get received_units() {
        const ret = wasm.sqlsyncresult_received_units(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get sent_changes() {
        const ret = wasm.sqlsyncresult_sent_changes(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get sent_snapshots() {
        const ret = wasm.sqlsyncresult_sent_snapshots(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get sent_units() {
        const ret = wasm.sqlsyncresult_sent_units(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) SqlSyncResult.prototype[Symbol.dispose] = SqlSyncResult.prototype.free;

/**
 * Sync role: Initiator or Responder.
 * @enum {0 | 1}
 */
export const SqlSyncRole = Object.freeze({
    Initiator: 0, "0": "Initiator",
    Responder: 1, "1": "Responder",
});

export class SqlTransaction {
    static __wrap(ptr) {
        const obj = Object.create(SqlTransaction.prototype);
        obj.__wbg_ptr = ptr;
        SqlTransactionFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SqlTransactionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_sqltransaction_free(ptr, 0);
    }
    /**
     * @returns {Promise<void>}
     */
    commit() {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.sqltransaction_commit(ptr);
        return ret;
    }
    /**
     * Execute SQL text within the transaction.
     * @param {string} sql
     * @param {string | null} [params]
     * @returns {Promise<string>}
     */
    execute(sql, params) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(sql, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = isLikeNone(params) ? 0 : passStringToWasm0(params, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.sqltransaction_execute(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @returns {Promise<void>}
     */
    rollback() {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.sqltransaction_rollback(ptr);
        return ret;
    }
}
if (Symbol.dispose) SqlTransaction.prototype[Symbol.dispose] = SqlTransaction.prototype.free;

/**
 * @returns {Promise<void>}
 */
export function init() {
    wasm.init();
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_208446807d0d1e30: function(arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg___wbindgen_debug_string_f249a2a76caa9a55: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_741ff2bb612836f5: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_undefined_be5b34249ff1fc15: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_string_get_7ce8eee7fd327545: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_4c88d8bf264fd984: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_09736879a3f67559: function(arg0) {
            arg0._wbg_cb_unref();
        },
        __wbg_call_40b008ba32f73991: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.call(arg1);
            return ret;
        }, arguments); },
        __wbg_call_a410eb4b59ed5965: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_clearTimeout_113b1cde814ec762: function(arg0) {
            const ret = clearTimeout(arg0);
            return ret;
        },
        __wbg_commit_e67cebcf3702bb67: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            const ret = commit(arg0, getStringFromWasm0(arg1, arg2), arg3, arg4);
            return ret;
        }, arguments); },
        __wbg_ensureOpen_eb3b6c4109282bf5: function() { return handleError(function (arg0) {
            ensureOpen(arg0);
        }, arguments); },
        __wbg_error_757e9472f8410341: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_from_eac91875c8a5ffa4: function(arg0) {
            const ret = Array.from(arg0);
            return ret;
        },
        __wbg_getRandomValues_19b7c67932c07470: function() { return handleError(function (arg0, arg1) {
            globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
        }, arguments); },
        __wbg_getRandomValues_8b6f1f663e6f188d: function() { return handleError(function (arg0, arg1) {
            globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
        }, arguments); },
        __wbg_get_2080c49015e35254: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_get_d7176cfb76d71114: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        },
        __wbg_get_unchecked_e6325ad374eff861: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        },
        __wbg_instanceof_ArrayBuffer_12cdaa4161423e6b: function(arg0) {
            let result;
            try {
                result = arg0 instanceof ArrayBuffer;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Error_74e3cca0c4a69161: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Error;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Promise_59148c6049d0e3a0: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Promise;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Uint8Array_d7aeb0dcb3108fea: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Uint8Array;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_jskvsyncerror_new: function(arg0) {
            const ret = JsKvSyncError.__wrap(arg0);
            return ret;
        },
        __wbg_jskvsynctransporterror_new: function(arg0) {
            const ret = JsKvSyncTransportError.__wrap(arg0);
            return ret;
        },
        __wbg_jssqlsyncerror_new: function(arg0) {
            const ret = JsSqlSyncError.__wrap(arg0);
            return ret;
        },
        __wbg_kvdatabase_new: function(arg0) {
            const ret = KvDatabase.__wrap(arg0);
            return ret;
        },
        __wbg_kvtransaction_new: function(arg0) {
            const ret = KvTransaction.__wrap(arg0);
            return ret;
        },
        __wbg_length_186163212ffe79c3: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_length_b03a0fabb109b1e9: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_log_ff86bbb34c295e21: function(arg0) {
            console.log(arg0);
        },
        __wbg_message_43d8c8f1d543a349: function(arg0) {
            const ret = arg0.message;
            return ret;
        },
        __wbg_new_13de428334042d26: function(arg0) {
            const ret = new Uint8Array(arg0);
            return ret;
        },
        __wbg_new_37eb45aaf5eeb820: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_787b9fb330d55d78: function(arg0, arg1) {
            const ret = new Error(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_bb74001155cb8ca8: function() {
            const ret = new Error();
            return ret;
        },
        __wbg_new_from_slice_b8daf545a41c6aa0: function(arg0, arg1) {
            const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_typed_e09da9dae6ee5935: function(arg0, arg1) {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke___js_sys_6820bde13593f591___Function_fn_wasm_bindgen_d83f6c3fe0419bf2___JsValue_____wasm_bindgen_d83f6c3fe0419bf2___sys__Undefined___js_sys_6820bde13593f591___Function_fn_wasm_bindgen_d83f6c3fe0419bf2___JsValue_____wasm_bindgen_d83f6c3fe0419bf2___sys__Undefined_______true_(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = new Promise(cb0);
                return ret;
            } finally {
                state0.a = 0;
            }
        },
        __wbg_now_463ac581d2fc7ea3: function() {
            const ret = Date.now();
            return ret;
        },
        __wbg_now_92c09207a20dd1e3: function() { return handleError(function () {
            const ret = Date.now();
            return ret;
        }, arguments); },
        __wbg_open_bdab64900cfc0121: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = open(getStringFromWasm0(arg0, arg1), getStringFromWasm0(arg2, arg3));
            return ret;
        }, arguments); },
        __wbg_prototypesetcall_13162965d1fe7aa3: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
        },
        __wbg_push_c9f4893fc209f768: function(arg0, arg1) {
            const ret = arg0.push(arg1);
            return ret;
        },
        __wbg_queueMicrotask_1e8503a8a690f0a3: function(arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        },
        __wbg_queueMicrotask_4a1a4d8dd332373b: function(arg0) {
            queueMicrotask(arg0);
        },
        __wbg_resolve_dbf78ab4942478b0: function(arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        },
        __wbg_setTimeout_a60d83bec06b4345: function() { return handleError(function (arg0, arg1) {
            const ret = setTimeout(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_set_2793479888329cb6: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_snapshot_ca04f0a951463215: function() { return handleError(function (arg0) {
            const ret = snapshot(arg0);
            return ret;
        }, arguments); },
        __wbg_sqldatabase_new: function(arg0) {
            const ret = SqlDatabase.__wrap(arg0);
            return ret;
        },
        __wbg_sqlsyncresult_new: function(arg0) {
            const ret = SqlSyncResult.__wrap(arg0);
            return ret;
        },
        __wbg_sqltransaction_new: function(arg0) {
            const ret = SqlTransaction.__wrap(arg0);
            return ret;
        },
        __wbg_stack_575c4e0918581b9c: function(arg0, arg1) {
            const ret = arg1.stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_static_accessor_GLOBAL_THIS_003ee833af9699fc: function() {
            const ret = typeof globalThis === 'undefined' ? undefined : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_e4293fb22cbe77da: function() {
            const ret = typeof global === 'undefined' ? undefined : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_53bb20457401453d: function() {
            const ret = typeof self === 'undefined' ? undefined : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_b5e0810bfecb7feb: function() {
            const ret = typeof window === 'undefined' ? undefined : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_then_656f253a5dd7d8ea: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_then_e3f7f2d365708be8: function(arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 2701, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke___wasm_bindgen_d83f6c3fe0419bf2___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_d83f6c3fe0419bf2___JsError___true_);
            return ret;
        },
        __wbindgen_generic_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [], shim_idx: 2693, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke_______true_);
            return ret;
        },
        __wbindgen_generic_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            table.set(0, undefined);
            table.set(1024, undefined);
            table.set(1025, null);
            table.set(1026, true);
            table.set(1027, false);
        },
    };
    return {
        __proto__: null,
        "./ofdb_wasm_bg.js": import0,
        "./snippets/ofdb-btree-indexeddb-8bc291a7517dcebb/src/indexeddb.js": import1,
    };
}

function wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke_______true_(arg0, arg1) {
    wasm.wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke_______true_(arg0, arg1);
}

function wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke___wasm_bindgen_d83f6c3fe0419bf2___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_d83f6c3fe0419bf2___JsError___true_(arg0, arg1, arg2) {
    const ret = wasm.wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke___wasm_bindgen_d83f6c3fe0419bf2___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_d83f6c3fe0419bf2___JsError___true_(arg0, arg1, arg2);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

function wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke___js_sys_6820bde13593f591___Function_fn_wasm_bindgen_d83f6c3fe0419bf2___JsValue_____wasm_bindgen_d83f6c3fe0419bf2___sys__Undefined___js_sys_6820bde13593f591___Function_fn_wasm_bindgen_d83f6c3fe0419bf2___JsValue_____wasm_bindgen_d83f6c3fe0419bf2___sys__Undefined_______true_(arg0, arg1, arg2, arg3) {
    wasm.wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke___js_sys_6820bde13593f591___Function_fn_wasm_bindgen_d83f6c3fe0419bf2___JsValue_____wasm_bindgen_d83f6c3fe0419bf2___sys__Undefined___js_sys_6820bde13593f591___Function_fn_wasm_bindgen_d83f6c3fe0419bf2___JsValue_____wasm_bindgen_d83f6c3fe0419bf2___sys__Undefined_______true_(arg0, arg1, arg2, arg3);
}

const JsKvChangeWorkFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_jskvchangework_free(ptr, 1));
const JsKvSyncErrorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_jskvsyncerror_free(ptr, 1));
const JsKvSyncTransportFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_jskvsynctransport_free(ptr, 1));
const JsKvSyncTransportErrorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_jskvsynctransporterror_free(ptr, 1));
const JsSqlSyncErrorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_jssqlsyncerror_free(ptr, 1));
const JsSqlSyncTransportFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_jssqlsynctransport_free(ptr, 1));
const JsSqlSyncTransportErrorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_jssqlsynctransporterror_free(ptr, 1));
const KvChangeSubscriptionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_kvchangesubscription_free(ptr, 1));
const KvClientFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_kvclient_free(ptr, 1));
const KvDatabaseFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_kvdatabase_free(ptr, 1));
const KvSyncConfigFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_kvsyncconfig_free(ptr, 1));
const KvTransactionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_kvtransaction_free(ptr, 1));
const SqlClientFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_sqlclient_free(ptr, 1));
const SqlDatabaseFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_sqldatabase_free(ptr, 1));
const SqlSessionConfigFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_sqlsessionconfig_free(ptr, 1));
const SqlSyncResultFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_sqlsyncresult_free(ptr, 1));
const SqlTransactionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_sqltransaction_free(ptr, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function _assertClass(instance, klass) {
    if (!(instance instanceof klass)) {
        throw new Error(`expected instance of ${klass.name}`);
    }
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure(state.a, state.b));

function debugString(val) {
    switch (typeof val) {
        // primitive types
        case 'number':
        case 'boolean':
        case 'undefined':
        return `${val}`;
        case 'string':
        return JSON.stringify(val);
        case 'bigint':
        return `${val}n`;
        case 'symbol':
        return val.description !== undefined
            ? `Symbol(${val.description})`
            : 'Symbol';
        case 'function':
        return typeof val.name === 'string' && val.name !== ''
            ? `Function(${val.name})`
            : 'Function';
        case 'object': {
            if (val === null) {
                return 'null';
            }
            // objects
            if (Array.isArray(val)) {
                return '[' + Array.from(val, debugString).join(', ') + ']';
            }
            // errors
            if (val instanceof Error) {
                return `${val.name}: ${val.message}\n${val.stack}`;
            }

            const toStringOutput = toString.call(val);
            // Test for built-in
            const className = /\[object ([^\]]+)\]/.exec(toStringOutput)?.[1];

            // Failed to match the standard '[object ClassName]'
            if (className === undefined) {
                return toStringOutput;
            }
            if (className === 'Object') {
                // we're a user defined class or Object
                // JSON.stringify avoids problems with cycles, and is generally much
                // easier than looping through ownProperties of `val`.
                try {
                    return 'Object(' + JSON.stringify(val) + ')';
                } catch (_) {
                    return 'Object';
                }
            }
            // TODO we could test for more things here, like `Set`s and `Map`s.
            return className;
        }
        // Per MDN: 'This list of values is exhaustive.'
        // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/typeof#description
    }
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeMutClosure(arg0, arg1, f) {
    const state = { a: arg0, b: arg1, cnt: 1 };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            wasm.__wbindgen_destroy_closure(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(bytes) {
    numBytesDecoded += bytes.length;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
        cachedTextDecoder.decode();
        numBytesDecoded = bytes.length;
    }
    return cachedTextDecoder.decode(bytes);
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

let __wbg_init_promise;
async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;
    if (__wbg_init_promise === undefined) {
        __wbg_init_promise = __wbg_init_once(module_or_path);
        __wbg_init_promise.catch(() => { __wbg_init_promise = undefined; });
    }
    return __wbg_init_promise;
}

async function __wbg_init_once(module_or_path) {

    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('ofdb_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
