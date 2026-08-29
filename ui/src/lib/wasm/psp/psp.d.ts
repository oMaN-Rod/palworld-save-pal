/* tslint:disable */
/* eslint-disable */

export function dispatch_frame(frame_json: string): Promise<void>;

export function export_gvas_file(name: string): Promise<Uint8Array>;

/**
 * `{ world_name, names: string[] }` — the download zip's file list. Small by
 * construction; the bytes come one at a time from `export_gvas_file`.
 */
export function export_gvas_manifest(): Promise<any>;

export function init(): void;

/**
 * `entries` is a JS array of `[filename, jsonText]` pairs.
 */
export function init_game_data(entries: any): void;

/**
 * The reverse direction, writing PlM/Oodle through the bridged codec.
 */
export function json_to_sav(json: string): Uint8Array;

export function load_staged_gvas(save_id: string): Promise<void>;

/**
 * Runs the schema migrations through the driver, then seeds the bundled
 * plugin set. The worker calls this after `set_sql_bridge` and before
 * dispatching frames -- both steps need the SQL bridge already wired, which
 * is why this isn't done inside `init()`.
 */
export function run_migrations(): Promise<void>;

/**
 * The raw editor's `.sav` -> uesave JSON direction. Stateless -- it reads no
 * session and stages nothing -- so it is safe to run while a save is loaded.
 */
export function sav_to_json(sav_bytes: Uint8Array): string;

export function set_emit_callback(cb: Function): void;

/**
 * Lends the engine the worker's `ooz.wasm` Oodle codec, which wasm32 cannot
 * link for itself. `compress(Uint8Array) -> Uint8Array` and
 * `decompress(Uint8Array, uncompressedLength) -> Uint8Array`, both synchronous:
 * the engine calls them from inside a save encode, so the module behind them
 * must already be up.
 */
export function set_oodle_bridge(compress: Function, decompress: Function): void;

export function set_sql_bridge(exec: Function, query: Function): void;

/**
 * `slot` is one of `level`, `level_meta`, `world_option`, `player_sav`,
 * `player_dps`; `uid` is ignored except for the two player slots. `bytes`
 * arrives as a `Uint8Array` and is moved straight into the staging area, so
 * the caller can drop its own copy immediately.
 */
export function stage_gvas(slot: string, uid: string, bytes: Uint8Array): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly dispatch_frame: (a: number, b: number) => any;
    readonly export_gvas_file: (a: number, b: number) => any;
    readonly export_gvas_manifest: () => any;
    readonly init: () => void;
    readonly init_game_data: (a: any) => [number, number];
    readonly json_to_sav: (a: number, b: number) => [number, number, number, number];
    readonly load_staged_gvas: (a: number, b: number) => any;
    readonly run_migrations: () => any;
    readonly sav_to_json: (a: number, b: number) => [number, number, number, number];
    readonly set_oodle_bridge: (a: any, b: any) => void;
    readonly set_sql_bridge: (a: any, b: any) => void;
    readonly stage_gvas: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly set_emit_callback: (a: any) => void;
    readonly __imported_wasi_snapshot_preview1_clock_time_get: (a: number, b: bigint, c: number) => number;
    readonly __imported_wasi_snapshot_preview1_environ_get: (a: number, b: number) => number;
    readonly __imported_wasi_snapshot_preview1_environ_sizes_get: (a: number, b: number) => number;
    readonly __imported_wasi_snapshot_preview1_fd_close: (a: number) => number;
    readonly __imported_wasi_snapshot_preview1_fd_fdstat_get: (a: number, b: number) => number;
    readonly __imported_wasi_snapshot_preview1_fd_prestat_dir_name: (a: number, b: number, c: number) => number;
    readonly __imported_wasi_snapshot_preview1_fd_prestat_get: (a: number, b: number) => number;
    readonly __imported_wasi_snapshot_preview1_fd_read: (a: number, b: number, c: number, d: number) => number;
    readonly __imported_wasi_snapshot_preview1_fd_seek: (a: number, b: bigint, c: number, d: number) => number;
    readonly __imported_wasi_snapshot_preview1_path_open: (a: number, b: number, c: number, d: number, e: number, f: bigint, g: bigint, h: number, i: number) => number;
    readonly __imported_wasi_snapshot_preview1_proc_exit: (a: number) => void;
    readonly clock: () => bigint;
    readonly system: (a: number) => number;
    readonly tmpfile: () => number;
    readonly tmpnam: (a: number) => number;
    readonly __imported_wasi_snapshot_preview1_fd_fdstat_set_flags: (a: number, b: number) => number;
    readonly __imported_wasi_snapshot_preview1_fd_renumber: (a: number, b: number) => number;
    readonly __imported_wasi_snapshot_preview1_fd_write: (a: number, b: number, c: number, d: number) => number;
    readonly wasm_bindgen_b846db123b7e88a6___convert__closures_____invoke___wasm_bindgen_b846db123b7e88a6___JsValue__core_9b3796e30d99ddb7___result__Result_____wasm_bindgen_b846db123b7e88a6___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_b846db123b7e88a6___convert__closures_____invoke___js_sys_e479fbbe3a4cd7bb___Function_fn_wasm_bindgen_b846db123b7e88a6___JsValue_____wasm_bindgen_b846db123b7e88a6___sys__Undefined___js_sys_e479fbbe3a4cd7bb___Function_fn_wasm_bindgen_b846db123b7e88a6___JsValue_____wasm_bindgen_b846db123b7e88a6___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly __wbindgen_export: WebAssembly.Table;
    readonly __wbindgen_malloc_command_export: (a: number, b: number) => number;
    readonly __wbindgen_realloc_command_export: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free_command_export: (a: number, b: number, c: number) => void;
    readonly __externref_table_alloc_command_export: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure_command_export: (a: number, b: number) => void;
    readonly __externref_table_dealloc_command_export: (a: number) => void;
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
