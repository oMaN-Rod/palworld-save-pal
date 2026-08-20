/* tslint:disable */
/* eslint-disable */

export function dispatch_frame(frame_json: string): Promise<void>;

/**
 * Serializes one manifest entry and hands it over as a `Uint8Array`.
 */
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
 * Loads everything staged so far and empties the staging area.
 */
export function load_staged_gvas(save_id: string): Promise<void>;

/**
 * Runs the schema migrations through the driver. The worker calls this after
 * `set_sql_bridge` and before dispatching frames.
 */
export function run_migrations(): Promise<void>;

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
    readonly dispatch_frame: (a: number, b: number) => number;
    readonly export_gvas_file: (a: number, b: number) => number;
    readonly export_gvas_manifest: () => number;
    readonly init: () => void;
    readonly init_game_data: (a: number, b: number) => void;
    readonly load_staged_gvas: (a: number, b: number) => number;
    readonly run_migrations: () => number;
    readonly set_oodle_bridge: (a: number, b: number) => void;
    readonly set_sql_bridge: (a: number, b: number) => void;
    readonly stage_gvas: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly set_emit_callback: (a: number) => void;
    readonly __wasm_bindgen_func_elem_5205: (a: number, b: number, c: number, d: number) => void;
    readonly __wasm_bindgen_func_elem_5219: (a: number, b: number, c: number, d: number) => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number) => void;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export5: (a: number, b: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
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
