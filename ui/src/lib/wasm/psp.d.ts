declare module '$lib/wasm/psp' {
	// wasm-pack's default export is the wasm loader; the rest are our exports.
	const initWasm: () => Promise<void>;
	export default initWasm;
	export function init(): void;
	export function set_emit_callback(cb: (frame: string) => void): void;
	export function init_game_data(entries: [string, string][]): void;
	export function dispatch_frame(frame: string): Promise<void>;
}
