/* @ts-self-types="./psp.d.ts" */

/**
 * @param {string} frame_json
 * @returns {Promise<void>}
 */
export function dispatch_frame(frame_json) {
    const ptr0 = passStringToWasm0(frame_json, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
    const len0 = WASM_VECTOR_LEN;
    let ret;
    __wbg_call_guard();
    try {
        ret = wasm.dispatch_frame(ptr0, len0);
    } catch(e) {
        __wbg_handle_catch(e);
    }
    return ret;
}

/**
 * @param {string} name
 * @returns {Promise<Uint8Array>}
 */
export function export_gvas_file(name) {
    const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
    const len0 = WASM_VECTOR_LEN;
    let ret;
    __wbg_call_guard();
    try {
        ret = wasm.export_gvas_file(ptr0, len0);
    } catch(e) {
        __wbg_handle_catch(e);
    }
    return ret;
}

/**
 * `{ world_name, names: string[] }` — the download zip's file list. Small by
 * construction; the bytes come one at a time from `export_gvas_file`.
 * @returns {Promise<any>}
 */
export function export_gvas_manifest() {
    let ret;
    __wbg_call_guard();
    try {
        ret = wasm.export_gvas_manifest();
    } catch(e) {
        __wbg_handle_catch(e);
    }
    return ret;
}

export function init() {
    __wbg_call_guard();
    try {
        wasm.init();
    } catch(e) {
        __wbg_handle_catch(e);
    }
}

/**
 * `entries` is a JS array of `[filename, jsonText]` pairs.
 * @param {any} entries
 */
export function init_game_data(entries) {
    let ret;
    __wbg_call_guard();
    try {
        ret = wasm.init_game_data(entries);
    } catch(e) {
        __wbg_handle_catch(e);
    }
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

/**
 * The reverse direction, writing PlM/Oodle through the bridged codec.
 * @param {string} json
 * @returns {Uint8Array}
 */
export function json_to_sav(json) {
    const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
    const len0 = WASM_VECTOR_LEN;
    let ret;
    __wbg_call_guard();
    try {
        ret = wasm.json_to_sav(ptr0, len0);
    } catch(e) {
        __wbg_handle_catch(e);
    }
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free_command_export(ret[0], ret[1] * 1, 1);
    return v2;
}

/**
 * @param {string} save_id
 * @returns {Promise<void>}
 */
export function load_staged_gvas(save_id) {
    const ptr0 = passStringToWasm0(save_id, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
    const len0 = WASM_VECTOR_LEN;
    let ret;
    __wbg_call_guard();
    try {
        ret = wasm.load_staged_gvas(ptr0, len0);
    } catch(e) {
        __wbg_handle_catch(e);
    }
    return ret;
}

/**
 * Runs the schema migrations through the driver, then seeds the bundled
 * plugin set. The worker calls this after `set_sql_bridge` and before
 * dispatching frames -- both steps need the SQL bridge already wired, which
 * is why this isn't done inside `init()`.
 * @returns {Promise<void>}
 */
export function run_migrations() {
    let ret;
    __wbg_call_guard();
    try {
        ret = wasm.run_migrations();
    } catch(e) {
        __wbg_handle_catch(e);
    }
    return ret;
}

/**
 * The raw editor's `.sav` -> uesave JSON direction. Stateless -- it reads no
 * session and stages nothing -- so it is safe to run while a save is loaded.
 * @param {Uint8Array} sav_bytes
 * @returns {string}
 */
export function sav_to_json(sav_bytes) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passArray8ToWasm0(sav_bytes, wasm.__wbindgen_malloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        let ret;
        __wbg_call_guard();
        try {
            ret = wasm.sav_to_json(ptr0, len0);
        } catch(e) {
            __wbg_handle_catch(e);
        }
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        __wbg_call_guard();
        try {
            wasm.__wbindgen_free_command_export(deferred3_0, deferred3_1, 1);
        } catch(e) {
            __wbg_handle_catch(e);
        }
    }
}

/**
 * @param {Function} cb
 */
export function set_emit_callback(cb) {
    __wbg_call_guard();
    try {
        wasm.set_emit_callback(cb);
    } catch(e) {
        __wbg_handle_catch(e);
    }
}

/**
 * Lends the engine the worker's `ooz.wasm` Oodle codec, which wasm32 cannot
 * link for itself. `compress(Uint8Array) -> Uint8Array` and
 * `decompress(Uint8Array, uncompressedLength) -> Uint8Array`, both synchronous:
 * the engine calls them from inside a save encode, so the module behind them
 * must already be up.
 * @param {Function} compress
 * @param {Function} decompress
 */
export function set_oodle_bridge(compress, decompress) {
    __wbg_call_guard();
    try {
        wasm.set_oodle_bridge(compress, decompress);
    } catch(e) {
        __wbg_handle_catch(e);
    }
}

/**
 * @param {Function} exec
 * @param {Function} query
 */
export function set_sql_bridge(exec, query) {
    __wbg_call_guard();
    try {
        wasm.set_sql_bridge(exec, query);
    } catch(e) {
        __wbg_handle_catch(e);
    }
}

/**
 * `slot` is one of `level`, `level_meta`, `world_option`, `player_sav`,
 * `player_dps`; `uid` is ignored except for the two player slots. `bytes`
 * arrives as a `Uint8Array` and is moved straight into the staging area, so
 * the caller can drop its own copy immediately.
 * @param {string} slot
 * @param {string} uid
 * @param {Uint8Array} bytes
 */
export function stage_gvas(slot, uid, bytes) {
    const ptr0 = passStringToWasm0(slot, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(uid, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc_command_export);
    const len2 = WASM_VECTOR_LEN;
    let ret;
    __wbg_call_guard();
    try {
        ret = wasm.stage_gvas(ptr0, len0, ptr1, len1, ptr2, len2);
    } catch(e) {
        __wbg_handle_catch(e);
    }
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_92b29b0548f8b746: function() { return wrapError(function (arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg___wbindgen_boolean_get_fa956cfa2d1bd751: function() { return wrapError(function (arg0) {
            const v = arg0;
            const ret = typeof(v) === 'boolean' ? v : undefined;
            return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
        }, arguments); },
        __wbg___wbindgen_debug_string_c25d447a39f5578f: function() { return wrapError(function (arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg___wbindgen_is_function_1ff95bcc5517c252: function() { return wrapError(function (arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        }, arguments); },
        __wbg___wbindgen_is_null_ea9085d691f535d3: function() { return wrapError(function (arg0) {
            const ret = arg0 === null;
            return ret;
        }, arguments); },
        __wbg___wbindgen_is_object_a27215656b807791: function() { return wrapError(function (arg0) {
            const val = arg0;
            const ret = typeof(val) === 'object' && val !== null;
            return ret;
        }, arguments); },
        __wbg___wbindgen_is_undefined_c05833b95a3cf397: function() { return wrapError(function (arg0) {
            const ret = arg0 === undefined;
            return ret;
        }, arguments); },
        __wbg___wbindgen_jsval_loose_eq_db4c3b15f63fc170: function() { return wrapError(function (arg0, arg1) {
            const ret = arg0 == arg1;
            return ret;
        }, arguments); },
        __wbg___wbindgen_number_get_394265ed1e1b84ee: function() { return wrapError(function (arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        }, arguments); },
        __wbg___wbindgen_string_get_b0ca35b86a603356: function() { return wrapError(function (arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg___wbindgen_throw_344f42d3211c4765: function() { return wrapError(function (arg0, arg1) {
            throw new WebAssembly.Exception(__wbindgen_wrapped_jstag, [new Error(getStringFromWasm0(arg0, arg1))]);
        }, arguments); },
        __wbg__wbg_cb_unref_fffb441def202758: function() { return wrapError(function (arg0) {
            arg0._wbg_cb_unref();
        }, arguments); },
        __wbg_apply_3ac86a26fdb56c05: function() { return wrapError(function (arg0, arg1, arg2) {
            const ret = arg0.apply(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_call_8a2dd23819f8a60a: function() { return wrapError(function (arg0, arg1) {
            const ret = arg0.call(arg1);
            return ret;
        }, arguments); },
        __wbg_call_a6e5c5dce5018821: function() { return wrapError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_call_e3b662382210db98: function() { return wrapError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.call(arg1, arg2, arg3);
            return ret;
        }, arguments); },
        __wbg_done_89b2b13e91a60321: function() { return wrapError(function (arg0) {
            const ret = arg0.done;
            return ret;
        }, arguments); },
        __wbg_error_a6fa202b58aa1cd3: function() { return wrapError(function (arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                __wbg_call_guard();
                try {
                    wasm.__wbindgen_free_command_export(deferred0_0, deferred0_1, 1);
                } catch(e) {
                    __wbg_handle_catch(e);
                }
            }
        }, arguments); },
        __wbg_from_13e323c65fc8f464: function() { return wrapError(function (arg0) {
            const ret = Array.from(arg0);
            return ret;
        }, arguments); },
        __wbg_getRandomValues_bf16787eede473f5: function() { return wrapError(function (arg0, arg1) {
            globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
        }, arguments); },
        __wbg_getTime_d6f070c088c9b5ed: function() { return wrapError(function (arg0) {
            const ret = arg0.getTime();
            return ret;
        }, arguments); },
        __wbg_getTimezoneOffset_dc9862c79e5a81a3: function() { return wrapError(function (arg0) {
            const ret = arg0.getTimezoneOffset();
            return ret;
        }, arguments); },
        __wbg_get_78f252d074a84d0b: function() { return wrapError(function (arg0, arg1) {
            const ret = Reflect.get(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_get_c7eb1f358a7654df: function() { return wrapError(function (arg0, arg1) {
            const ret = Reflect.get(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_get_unchecked_6e0ad6d2a41b06f6: function() { return wrapError(function (arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        }, arguments); },
        __wbg_instanceof_ArrayBuffer_4480b9e0068a8adb: function() { return wrapError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof ArrayBuffer;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        }, arguments); },
        __wbg_instanceof_Uint8Array_309b927aaf7a3fc7: function() { return wrapError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof Uint8Array;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        }, arguments); },
        __wbg_isArray_0677c962b281d01a: function() { return wrapError(function (arg0) {
            const ret = Array.isArray(arg0);
            return ret;
        }, arguments); },
        __wbg_iterator_6f722e4a93058b71: function() { return wrapError(function () {
            const ret = Symbol.iterator;
            return ret;
        }, arguments); },
        __wbg_keys_58421f8f96795607: function() { return wrapError(function (arg0) {
            const ret = Object.keys(arg0);
            return ret;
        }, arguments); },
        __wbg_length_1f0964f4a5e2c6d8: function() { return wrapError(function (arg0) {
            const ret = arg0.length;
            return ret;
        }, arguments); },
        __wbg_length_370319915dc99107: function() { return wrapError(function (arg0) {
            const ret = arg0.length;
            return ret;
        }, arguments); },
        __wbg_new_0_3da9e97f24fc69be: function() { return wrapError(function () {
            const ret = new Date();
            return ret;
        }, arguments); },
        __wbg_new_227d7c05414eb861: function() { return wrapError(function () {
            const ret = new Error();
            return ret;
        }, arguments); },
        __wbg_new_32b398fb48b6d94a: function() { return wrapError(function () {
            const ret = new Array();
            return ret;
        }, arguments); },
        __wbg_new_cc984128914cfc6f: function() { return wrapError(function (arg0) {
            const ret = new Date(arg0);
            return ret;
        }, arguments); },
        __wbg_new_cd45aabdf6073e84: function() { return wrapError(function (arg0) {
            const ret = new Uint8Array(arg0);
            return ret;
        }, arguments); },
        __wbg_new_da52cf8fe3429cb2: function() { return wrapError(function () {
            const ret = new Object();
            return ret;
        }, arguments); },
        __wbg_new_from_slice_77cdfb7977362f3c: function() { return wrapError(function (arg0, arg1) {
            const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_new_typed_1824d93f294193e5: function() { return wrapError(function (arg0, arg1) {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen_b846db123b7e88a6___convert__closures_____invoke___js_sys_e479fbbe3a4cd7bb___Function_fn_wasm_bindgen_b846db123b7e88a6___JsValue_____wasm_bindgen_b846db123b7e88a6___sys__Undefined___js_sys_e479fbbe3a4cd7bb___Function_fn_wasm_bindgen_b846db123b7e88a6___JsValue_____wasm_bindgen_b846db123b7e88a6___sys__Undefined_______true_(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = new Promise(cb0);
                return ret;
            } finally {
                state0.a = 0;
            }
        }, arguments); },
        __wbg_next_6dbf2c0ac8cde20f: function() { return wrapError(function (arg0) {
            const ret = arg0.next;
            return ret;
        }, arguments); },
        __wbg_next_71f2aa1cb3d1e37e: function() { return wrapError(function (arg0) {
            const ret = arg0.next();
            return ret;
        }, arguments); },
        __wbg_prototypesetcall_4770620bbe4688a0: function() { return wrapError(function (arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
        }, arguments); },
        __wbg_push_d2ae3af0c1217ae6: function() { return wrapError(function (arg0, arg1) {
            const ret = arg0.push(arg1);
            return ret;
        }, arguments); },
        __wbg_queueMicrotask_0ab5b2d2393e99b9: function() { return wrapError(function (arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        }, arguments); },
        __wbg_queueMicrotask_6a09b7bc46549209: function() { return wrapError(function (arg0) {
            queueMicrotask(arg0);
        }, arguments); },
        __wbg_resolve_2191a4dfe481c25b: function() { return wrapError(function (arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        }, arguments); },
        __wbg_set_8535240470bf2500: function() { return wrapError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_stack_3b0d974bbf31e44f: function() { return wrapError(function (arg0, arg1) {
            const ret = arg1.stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_static_accessor_GLOBAL_4ef717fb391d88b7: function() { return wrapError(function () {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_static_accessor_GLOBAL_THIS_8d1badc68b5a74f4: function() { return wrapError(function () {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_static_accessor_SELF_146583524fe1469b: function() { return wrapError(function () {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_static_accessor_WINDOW_f2829a2234d7819e: function() { return wrapError(function () {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_then_6ec10ae38b3e92f7: function() { return wrapError(function (arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        }, arguments); },
        __wbg_value_a5d5488a9589444a: function() { return wrapError(function (arg0) {
            const ret = arg0.value;
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000001: function() { return wrapError(function (arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 1731, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_b846db123b7e88a6___convert__closures_____invoke___wasm_bindgen_b846db123b7e88a6___JsValue__core_9b3796e30d99ddb7___result__Result_____wasm_bindgen_b846db123b7e88a6___JsError___true_);
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000002: function() { return wrapError(function (arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000003: function() { return wrapError(function (arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000004: function() { return wrapError(function (arg0, arg1) {
            var v0 = getArrayU8FromWasm0(arg0, arg1).slice();
            wasm.__wbindgen_free_command_export(arg0, arg1 * 1, 1);
            // Cast intrinsic for `Vector(U8) -> Externref`.
            const ret = v0;
            return ret;
        }, arguments); },
        __wbindgen_init_externref_table: function() { return wrapError(function () {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        }, arguments); },
        __wbindgen_jstag: __wbindgen_jstag_polyfill,
        __wbindgen_wrapped_jstag: __wbindgen_wrapped_jstag,
    };
    return {
        __proto__: null,
        "./psp_bg.js": import0,
    };
}

const __wbindgen_jstag_polyfill = new WebAssembly.Tag({ parameters: ['externref'] });


const __wbindgen_wrapped_jstag = new WebAssembly.Tag({ parameters: ['externref'] });


let __wbg_terminated_addr;
let __wbg_called_abort = false;
function __wbg_call_abort_hook() {
    __wbg_called_abort = true;
    try {
        const idx = getInt32ArrayMemory0()[wasm.__abort_handler.value / 4];
        if (idx) wasm.__wbindgen_export.get(idx)();
    } catch(_) {}
}

function __wbg_handle_catch(e) {
    if (e instanceof WebAssembly.Exception && e.is(__wbindgen_wrapped_jstag)) {
        throw e.getArg(__wbindgen_wrapped_jstag, 0);
    }
    getInt32ArrayMemory0()[__wbg_terminated_addr] = 1;
    __wbg_call_abort_hook();
    throw e;
}


function __wbg_call_guard() {
    __wbg_terminated_addr ??= wasm.__instance_terminated.value / 4;
    const flag = getInt32ArrayMemory0()[__wbg_terminated_addr];
    if (flag) {
        if (!__wbg_called_abort) {
            __wbg_call_abort_hook();
        }throw new Error('Module terminated');
    }
}
function wasm_bindgen_b846db123b7e88a6___convert__closures_____invoke___wasm_bindgen_b846db123b7e88a6___JsValue__core_9b3796e30d99ddb7___result__Result_____wasm_bindgen_b846db123b7e88a6___JsError___true_(arg0, arg1, arg2) {
    let ret;
    __wbg_call_guard();
    try {
        ret = wasm.wasm_bindgen_b846db123b7e88a6___convert__closures_____invoke___wasm_bindgen_b846db123b7e88a6___JsValue__core_9b3796e30d99ddb7___result__Result_____wasm_bindgen_b846db123b7e88a6___JsError___true_(arg0, arg1, arg2);
    } catch(e) {
        __wbg_handle_catch(e);
    }
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

function wasm_bindgen_b846db123b7e88a6___convert__closures_____invoke___js_sys_e479fbbe3a4cd7bb___Function_fn_wasm_bindgen_b846db123b7e88a6___JsValue_____wasm_bindgen_b846db123b7e88a6___sys__Undefined___js_sys_e479fbbe3a4cd7bb___Function_fn_wasm_bindgen_b846db123b7e88a6___JsValue_____wasm_bindgen_b846db123b7e88a6___sys__Undefined_______true_(arg0, arg1, arg2, arg3) {
    __wbg_call_guard();
    try {
        wasm.wasm_bindgen_b846db123b7e88a6___convert__closures_____invoke___js_sys_e479fbbe3a4cd7bb___Function_fn_wasm_bindgen_b846db123b7e88a6___JsValue_____wasm_bindgen_b846db123b7e88a6___sys__Undefined___js_sys_e479fbbe3a4cd7bb___Function_fn_wasm_bindgen_b846db123b7e88a6___JsValue_____wasm_bindgen_b846db123b7e88a6___sys__Undefined_______true_(arg0, arg1, arg2, arg3);
    } catch(e) {
        __wbg_handle_catch(e);
    }
}

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc_command_export();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure_command_export(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
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

let cachedInt32ArrayMemory0 = null;
function getInt32ArrayMemory0() {
    if (cachedInt32ArrayMemory0 === null || cachedInt32ArrayMemory0.byteLength === 0) {
        cachedInt32ArrayMemory0 = new Int32Array(wasm.memory.buffer);
    }
    return cachedInt32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
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
            wasm.__wbindgen_destroy_closure_command_export(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
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
    wasm.__externref_table_dealloc_command_export(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
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

function wrapError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        if (e instanceof WebAssembly.Exception) throw e;
        throw new WebAssembly.Exception(__wbindgen_jstag_polyfill, [e], { traceStack: true });
    }
}

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedInt32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

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

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('psp_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
