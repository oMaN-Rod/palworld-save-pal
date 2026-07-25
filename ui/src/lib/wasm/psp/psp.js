const NOT_BUILT = 'psp wasm not built; run build:web';

const initWasm = async () => {};
export default initWasm;

export function init() {
	throw new Error(NOT_BUILT);
}

export function set_emit_callback() {
	throw new Error(NOT_BUILT);
}

export function init_game_data() {
	throw new Error(NOT_BUILT);
}

export async function dispatch_frame() {
	throw new Error(NOT_BUILT);
}
