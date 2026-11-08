/* tslint:disable */
/* eslint-disable */

/**
 * An open File System in the browser.
 */
export class FileSystemHandle {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Announces metadata to every peer over the JavaScript bridge.
     */
    announce(bridge: FileTransport): Promise<void>;
    append(path: string, data: Uint8Array): Promise<void>;
    /**
     * Closes this filesystem, releasing its exclusive ownership lock.
     */
    close(): void;
    create_dir(path: string): Promise<any>;
    delete(path: string): Promise<void>;
    entry(path: string): Promise<any>;
    /**
     * Exports the metadata snapshot of this filesystem as JSON.
     */
    export_snapshots(): Promise<any>;
    /**
     * Fetches one file's content from its providers over the JavaScript bridge.
     */
    fetch(bridge: FileTransport, path: string): Promise<void>;
    list(path: string): Promise<any>;
    /**
     * Opens a browser File System rooted at `root` for one peer.
     */
    static open(root: string, node_id?: string | null): Promise<FileSystemHandle>;
    /**
     * Runs one pass of the metadata sync loop over the JavaScript bridge.
     */
    pump(bridge: FileTransport): Promise<void>;
    read(path: string): Promise<Uint8Array>;
    rename(from: string, to: string): Promise<void>;
    residency(path: string): Promise<string>;
    revision(path: string): Promise<string>;
    scan(path: string, cursor: string | null | undefined, limit: number): Promise<any>;
    set_residency(path: string, residency: string): Promise<void>;
    /**
     * Reconciles with peers until the filesystem is retired or the bridge closes.
     */
    sync(bridge: FileTransport): Promise<void>;
    write_at(path: string, expected_revision: string, offset: bigint, data: Uint8Array): Promise<number>;
    write(path: string, data: Uint8Array): Promise<any>;
}

export function init(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_filesystemhandle_free: (a: number, b: number) => void;
    readonly filesystemhandle_announce: (a: number, b: any) => any;
    readonly filesystemhandle_append: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly filesystemhandle_close: (a: number) => void;
    readonly filesystemhandle_create_dir: (a: number, b: number, c: number) => any;
    readonly filesystemhandle_delete: (a: number, b: number, c: number) => any;
    readonly filesystemhandle_entry: (a: number, b: number, c: number) => any;
    readonly filesystemhandle_export_snapshots: (a: number) => any;
    readonly filesystemhandle_fetch: (a: number, b: any, c: number, d: number) => any;
    readonly filesystemhandle_list: (a: number, b: number, c: number) => any;
    readonly filesystemhandle_open: (a: number, b: number, c: number, d: number) => any;
    readonly filesystemhandle_pump: (a: number, b: any) => any;
    readonly filesystemhandle_read: (a: number, b: number, c: number) => any;
    readonly filesystemhandle_rename: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly filesystemhandle_residency: (a: number, b: number, c: number) => any;
    readonly filesystemhandle_revision: (a: number, b: number, c: number) => any;
    readonly filesystemhandle_scan: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
    readonly filesystemhandle_set_residency: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly filesystemhandle_sync: (a: number, b: any) => any;
    readonly filesystemhandle_write: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly filesystemhandle_write_at: (a: number, b: number, c: number, d: number, e: number, f: bigint, g: number, h: number) => any;
    readonly init: () => void;
    readonly wasm_bindgen_cabfba5d4ac2a115___convert__closures_____invoke___wasm_bindgen_cabfba5d4ac2a115___JsValue__wasm_bindgen_cabfba5d4ac2a115___JsValue__js_sys_261284eb719a70e7___Function______true_: (a: number, b: number, c: any, d: any, e: any) => void;
    readonly wasm_bindgen_cabfba5d4ac2a115___convert__closures_____invoke___js_sys_261284eb719a70e7___Function_fn_wasm_bindgen_cabfba5d4ac2a115___JsValue_____wasm_bindgen_cabfba5d4ac2a115___sys__Undefined___js_sys_261284eb719a70e7___Function_fn_wasm_bindgen_cabfba5d4ac2a115___JsValue_____wasm_bindgen_cabfba5d4ac2a115___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_cabfba5d4ac2a115___convert__closures_____invoke___wasm_bindgen_cabfba5d4ac2a115___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_cabfba5d4ac2a115___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_cabfba5d4ac2a115___convert__closures_____invoke___wasm_bindgen_cabfba5d4ac2a115___JsValue__wasm_bindgen_cabfba5d4ac2a115___JsValue______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_cabfba5d4ac2a115___convert__closures_____invoke_______true_: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
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
