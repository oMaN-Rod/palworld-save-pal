// Hand-written types for the emscripten-generated ooz.mjs module.
export interface OozModule {
	/** Reassigned by the runtime whenever wasm memory grows — always access at point of use. */
	HEAPU8: Uint8Array;
	_malloc(size: number): number;
	_free(ptr: number): void;
	ccall(name: string, returnType: string, argTypes: string[], args: number[]): number;
}

declare function createOozModule(): Promise<OozModule>;
export default createOozModule;
