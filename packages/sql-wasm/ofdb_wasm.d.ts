/* tslint:disable */
/* eslint-disable */

/**
 * Opaque change work handle.
 */
export class JsKvChangeWork {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
}

/**
 * Sync error.
 */
export class JsKvSyncError {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly kind: JsKvSyncErrorKind;
    readonly message: string;
}

/**
 * Sync error kind.
 */
export enum JsKvSyncErrorKind {
    PreApply = 0,
    Transport = 1,
    Codec = 2,
    InvalidFrame = 3,
    FrameTooLarge = 4,
    UnsupportedVersion = 5,
    UnexpectedMessage = 6,
    Storage = 7,
}

/**
 * Sync transport trait that can be implemented in JavaScript.
 */
export class JsKvSyncTransport {
    free(): void;
    [Symbol.dispose](): void;
    constructor(transport: any);
}

/**
 * Transport error for JS-implemented transports.
 */
export class JsKvSyncTransportError {
    free(): void;
    [Symbol.dispose](): void;
    constructor(message: string);
    readonly message: string;
}

/**
 * Sync error.
 */
export class JsSqlSyncError {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly kind: JsSqlSyncErrorKind;
    readonly message: string;
}

/**
 * Sync error kind.
 */
export enum JsSqlSyncErrorKind {
    PreApply = 0,
    Engine = 1,
    Transport = 2,
    Staging = 3,
    InvalidConfiguration = 4,
    Protocol = 5,
    MessageTooLarge = 6,
    SessionTooLarge = 7,
    IncompatibleProtocol = 8,
    UnexpectedMessage = 9,
    RemoteAbort = 10,
}

/**
 * Sync transport trait that can be implemented in JavaScript.
 */
export class JsSqlSyncTransport {
    free(): void;
    [Symbol.dispose](): void;
    constructor(transport: any);
}

/**
 * Transport error for JS-implemented transports.
 */
export class JsSqlSyncTransportError {
    free(): void;
    [Symbol.dispose](): void;
    constructor(message: string);
    readonly message: string;
}

/**
 * Change subscription for continuous synchronization.
 */
export class KvChangeSubscription {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Acknowledge completed work.
     */
    acknowledge(work: JsKvChangeWork): boolean;
    /**
     * Wait for changes.
     */
    changed(): Promise<void>;
    /**
     * Merge with another subscription.
     */
    merge(other: KvChangeSubscription): void;
    /**
     * Select the next unit of work to synchronize.
     */
    select(): JsKvChangeWork | undefined;
}

export class KvClient {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Return an independent clone sharing the same storage.
     */
    clone_client(): KvClient;
    /**
     * Remove a committed value.
     */
    delete(key: string): Promise<void>;
    /**
     * Read a committed value as JSON; a missing key becomes JavaScript undefined.
     */
    get(key: string): Promise<string | undefined>;
    /**
     * Scan entries with keys in [start, end) as a JSON array of {key, value} objects.
     */
    scan(start: string, end: string): Promise<string>;
    /**
     * Scan all entries.
     */
    scan_all(): Promise<string>;
    /**
     * Scan entries whose keys start with `prefix`.
     */
    scan_prefix(prefix: string): Promise<string>;
    /**
     * Store a JSON value as the committed value, with optional expiry (Unix epoch millis).
     */
    set(key: string, value: string, expires_at?: bigint | null): Promise<void>;
    /**
     * Start a transaction that holds operations until commit or rollback.
     */
    transaction(): Promise<KvTransaction>;
    /**
     * Return a clone of this client with a request deadline in milliseconds.
     */
    with_deadline(ms: bigint): KvClient;
}

export class KvDatabase {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create a query handle sharing this database's storage.
     */
    client(): KvClient;
    /**
     * Open a key-value database backed by IndexedDB.
     */
    static open(name: string): Promise<KvDatabase>;
    /**
     * Get the change subscription for continuous synchronization.
     */
    sync_changes(): KvChangeSubscription;
    /**
     * Synchronize with a remote peer using the given transport.
     */
    synchronize(transport: JsKvSyncTransport, role: KvSyncRole, config: KvSyncConfig): Promise<void>;
    /**
     * Synchronize with a remote peer, checking caller authorization before each batch.
     */
    synchronize_checked(transport: JsKvSyncTransport, role: KvSyncRole, config: KvSyncConfig, check: Function): Promise<void>;
    /**
     * Synchronize selected changes with a remote peer, checking caller authorization.
     */
    synchronize_selected_checked(subscription: KvChangeSubscription, transport: JsKvSyncTransport, role: KvSyncRole, config: KvSyncConfig, check: Function): Promise<void>;
}

/**
 * Sync configuration.
 */
export class KvSyncConfig {
    free(): void;
    [Symbol.dispose](): void;
    constructor();
    max_frame_bytes: number;
    max_inventory_bytes: number;
    max_inventory_entries: number;
    max_snapshots_per_batch: number;
}

/**
 * Sync role: Initiator or Responder.
 */
export enum KvSyncRole {
    Initiator = 0,
    Responder = 1,
}

export class KvTransaction {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    commit(): Promise<void>;
    delete(key: string): Promise<void>;
    get(key: string): Promise<string | undefined>;
    rollback(): Promise<void>;
    scan(start: string, end: string): Promise<string>;
    scan_all(): Promise<string>;
    scan_prefix(prefix: string): Promise<string>;
    set(key: string, value: string, expires_at?: bigint | null): Promise<void>;
}

export class SqlClient {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Return an independent clone sharing the same storage.
     */
    clone_client(): SqlClient;
    /**
     * Execute SQL text with optional bound parameters and return results as JSON.
     *
     * Parameters are a JSON array for positional placeholders or a JSON object for
     * named placeholders. Every value uses the owner's lossless tagged codec.
     * The result is a JSON array of `{ columns, rows }` objects with tagged cells.
     */
    execute(sql: string, params?: string | null): Promise<string>;
    /**
     * Apply the complete application migration history through the Rust runner.
     */
    migrate(migrations: string): Promise<string>;
    /**
     * Start a transaction that holds statements until commit or rollback.
     */
    transaction(): Promise<SqlTransaction>;
    /**
     * Return a clone of this client with a request deadline in milliseconds.
     */
    with_deadline(ms: bigint): SqlClient;
}

export class SqlDatabase {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create a query handle sharing this database's storage.
     */
    client(): SqlClient;
    /**
     * Open a SQL database backed by IndexedDB.
     */
    static open(name: string): Promise<SqlDatabase>;
    /**
     * Synchronize with a remote peer using the given transport.
     */
    synchronize(transport: JsSqlSyncTransport, config: SqlSessionConfig, role: SqlSyncRole): Promise<SqlSyncResult>;
    /**
     * Synchronize with a remote peer, checking caller authorization before each batch.
     */
    synchronize_checked(transport: JsSqlSyncTransport, config: SqlSessionConfig, role: SqlSyncRole, check: Function): Promise<SqlSyncResult>;
}

/**
 * Sync configuration.
 */
export class SqlSessionConfig {
    free(): void;
    [Symbol.dispose](): void;
    constructor();
    max_session_bytes: number;
    max_units_per_frame: number;
}

/**
 * Sync direction.
 */
export enum SqlSyncDirection {
    Bidirectional = 0,
}

/**
 * Sync result.
 */
export class SqlSyncResult {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly received_changes: number;
    readonly received_snapshots: number;
    readonly received_units: number;
    readonly sent_changes: number;
    readonly sent_snapshots: number;
    readonly sent_units: number;
}

/**
 * Sync role: Initiator or Responder.
 */
export enum SqlSyncRole {
    Initiator = 0,
    Responder = 1,
}

export class SqlTransaction {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    commit(): Promise<void>;
    /**
     * Execute SQL text within the transaction.
     */
    execute(sql: string, params?: string | null): Promise<string>;
    rollback(): Promise<void>;
}

export function init(): Promise<void>;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_jskvchangework_free: (a: number, b: number) => void;
    readonly __wbg_jskvsyncerror_free: (a: number, b: number) => void;
    readonly __wbg_jskvsynctransport_free: (a: number, b: number) => void;
    readonly __wbg_jskvsynctransporterror_free: (a: number, b: number) => void;
    readonly __wbg_jssqlsyncerror_free: (a: number, b: number) => void;
    readonly __wbg_jssqlsynctransport_free: (a: number, b: number) => void;
    readonly __wbg_jssqlsynctransporterror_free: (a: number, b: number) => void;
    readonly __wbg_kvchangesubscription_free: (a: number, b: number) => void;
    readonly __wbg_kvclient_free: (a: number, b: number) => void;
    readonly __wbg_kvdatabase_free: (a: number, b: number) => void;
    readonly __wbg_kvsyncconfig_free: (a: number, b: number) => void;
    readonly __wbg_kvtransaction_free: (a: number, b: number) => void;
    readonly __wbg_sqlclient_free: (a: number, b: number) => void;
    readonly __wbg_sqldatabase_free: (a: number, b: number) => void;
    readonly __wbg_sqlsessionconfig_free: (a: number, b: number) => void;
    readonly __wbg_sqlsyncresult_free: (a: number, b: number) => void;
    readonly __wbg_sqltransaction_free: (a: number, b: number) => void;
    readonly init: () => void;
    readonly jskvsyncerror_kind: (a: number) => number;
    readonly jskvsyncerror_message: (a: number) => [number, number];
    readonly jskvsynctransport_new: (a: any) => number;
    readonly jskvsynctransporterror_message: (a: number) => [number, number];
    readonly jskvsynctransporterror_new: (a: number, b: number) => number;
    readonly jssqlsyncerror_kind: (a: number) => number;
    readonly jssqlsyncerror_message: (a: number) => [number, number];
    readonly jssqlsynctransport_new: (a: any) => number;
    readonly jssqlsynctransporterror_message: (a: number) => [number, number];
    readonly jssqlsynctransporterror_new: (a: number, b: number) => number;
    readonly kvchangesubscription_acknowledge: (a: number, b: number) => number;
    readonly kvchangesubscription_changed: (a: number) => any;
    readonly kvchangesubscription_merge: (a: number, b: number) => void;
    readonly kvchangesubscription_select: (a: number) => number;
    readonly kvclient_clone_client: (a: number) => number;
    readonly kvclient_delete: (a: number, b: number, c: number) => any;
    readonly kvclient_get: (a: number, b: number, c: number) => any;
    readonly kvclient_scan: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly kvclient_scan_all: (a: number) => any;
    readonly kvclient_scan_prefix: (a: number, b: number, c: number) => any;
    readonly kvclient_set: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => any;
    readonly kvclient_transaction: (a: number) => any;
    readonly kvclient_with_deadline: (a: number, b: bigint) => number;
    readonly kvdatabase_client: (a: number) => number;
    readonly kvdatabase_open: (a: number, b: number) => any;
    readonly kvdatabase_sync_changes: (a: number) => [number, number, number];
    readonly kvdatabase_synchronize: (a: number, b: number, c: number, d: number) => any;
    readonly kvdatabase_synchronize_checked: (a: number, b: number, c: number, d: number, e: any) => any;
    readonly kvdatabase_synchronize_selected_checked: (a: number, b: number, c: number, d: number, e: number, f: any) => any;
    readonly kvsyncconfig_max_frame_bytes: (a: number) => number;
    readonly kvsyncconfig_max_inventory_bytes: (a: number) => number;
    readonly kvsyncconfig_max_inventory_entries: (a: number) => number;
    readonly kvsyncconfig_max_snapshots_per_batch: (a: number) => number;
    readonly kvsyncconfig_new: () => number;
    readonly kvsyncconfig_set_max_frame_bytes: (a: number, b: number) => void;
    readonly kvsyncconfig_set_max_inventory_bytes: (a: number, b: number) => void;
    readonly kvsyncconfig_set_max_inventory_entries: (a: number, b: number) => void;
    readonly kvsyncconfig_set_max_snapshots_per_batch: (a: number, b: number) => void;
    readonly kvtransaction_commit: (a: number) => any;
    readonly kvtransaction_delete: (a: number, b: number, c: number) => any;
    readonly kvtransaction_get: (a: number, b: number, c: number) => any;
    readonly kvtransaction_rollback: (a: number) => any;
    readonly kvtransaction_scan: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly kvtransaction_scan_all: (a: number) => any;
    readonly kvtransaction_scan_prefix: (a: number, b: number, c: number) => any;
    readonly kvtransaction_set: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => any;
    readonly sqlclient_clone_client: (a: number) => number;
    readonly sqlclient_execute: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly sqlclient_migrate: (a: number, b: number, c: number) => any;
    readonly sqlclient_transaction: (a: number) => any;
    readonly sqlclient_with_deadline: (a: number, b: bigint) => number;
    readonly sqldatabase_client: (a: number) => number;
    readonly sqldatabase_open: (a: number, b: number) => any;
    readonly sqldatabase_synchronize: (a: number, b: number, c: number, d: number) => any;
    readonly sqldatabase_synchronize_checked: (a: number, b: number, c: number, d: number, e: any) => any;
    readonly sqlsessionconfig_max_session_bytes: (a: number) => number;
    readonly sqlsessionconfig_max_units_per_frame: (a: number) => number;
    readonly sqlsessionconfig_new: () => number;
    readonly sqlsessionconfig_set_max_session_bytes: (a: number, b: number) => void;
    readonly sqlsessionconfig_set_max_units_per_frame: (a: number, b: number) => void;
    readonly sqlsyncresult_received_changes: (a: number) => number;
    readonly sqlsyncresult_received_snapshots: (a: number) => number;
    readonly sqlsyncresult_received_units: (a: number) => number;
    readonly sqlsyncresult_sent_changes: (a: number) => number;
    readonly sqlsyncresult_sent_snapshots: (a: number) => number;
    readonly sqlsyncresult_sent_units: (a: number) => number;
    readonly sqltransaction_commit: (a: number) => any;
    readonly sqltransaction_execute: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly sqltransaction_rollback: (a: number) => any;
    readonly wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke___js_sys_6820bde13593f591___Function_fn_wasm_bindgen_d83f6c3fe0419bf2___JsValue_____wasm_bindgen_d83f6c3fe0419bf2___sys__Undefined___js_sys_6820bde13593f591___Function_fn_wasm_bindgen_d83f6c3fe0419bf2___JsValue_____wasm_bindgen_d83f6c3fe0419bf2___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke___wasm_bindgen_d83f6c3fe0419bf2___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_d83f6c3fe0419bf2___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_d83f6c3fe0419bf2___convert__closures_____invoke_______true_: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
