async function Module(moduleArg = {}) {
	var Module = moduleArg;
	var ENVIRONMENT_IS_WEB = !!globalThis.window;
	var ENVIRONMENT_IS_WORKER = !!globalThis.WorkerGlobalScope;
	var ENVIRONMENT_IS_NODE =
		globalThis.process?.versions?.node && globalThis.process?.type != 'renderer';
	if (ENVIRONMENT_IS_NODE) {
		const { createRequire } = await import('node:module');
		var require = createRequire(import.meta.url);
	}
	var programArgs = [];
	var thisProgram = './this.program';
	var quit_ = (status, toThrow) => {
		throw toThrow;
	};
	var _scriptName = import.meta.url;
	var scriptDirectory = '';
	function locateFile(path) {
		if (Module['locateFile']) {
			return Module['locateFile'](path, scriptDirectory);
		}
		return scriptDirectory + path;
	}
	var readAsync, readBinary;
	if (ENVIRONMENT_IS_NODE) {
		var fs = require('node:fs');
		if (_scriptName.startsWith('file:')) {
			scriptDirectory =
				require('node:path').dirname(require('node:url').fileURLToPath(_scriptName)) + '/';
		}
		readBinary = (filename) => {
			filename = isFileURI(filename) ? new URL(filename) : filename;
			var ret = fs.readFileSync(filename);
			return ret;
		};
		readAsync = async (filename, binary = true) => {
			filename = isFileURI(filename) ? new URL(filename) : filename;
			var ret = fs.readFileSync(filename, binary ? undefined : 'utf8');
			return ret;
		};
		if (process.argv.length > 1) {
			thisProgram = process.argv[1].replace(/\\/g, '/');
		}
		programArgs = process.argv.slice(2);
		quit_ = (status, toThrow) => {
			process.exitCode = status;
			throw toThrow;
		};
	} else if (ENVIRONMENT_IS_WEB || ENVIRONMENT_IS_WORKER) {
		try {
			scriptDirectory = new URL('.', _scriptName).href;
		} catch {}
		{
			if (ENVIRONMENT_IS_WORKER) {
				readBinary = (url) => {
					var xhr = new XMLHttpRequest();
					xhr.open('GET', url, false);
					xhr.responseType = 'arraybuffer';
					xhr.send(null);
					return new Uint8Array(xhr.response);
				};
			}
			readAsync = async (url) => {
				var response = await fetch(url, { credentials: 'same-origin' });
				if (response.ok) {
					return response.arrayBuffer();
				}
				throw new Error(response.status + ' : ' + response.url);
			};
		}
	} else {
	}
	var out = console.log.bind(console);
	var err = console.error.bind(console);
	var wasmBinary;
	var ABORT = false;
	var isFileURI = (filename) => filename.startsWith('file://');
	class EmscriptenEH {}
	class EmscriptenSjLj extends EmscriptenEH {}
	var runtimeInitialized = false;
	function getMemoryBuffer() {
		try {
			var b = wasmMemory.toResizableBuffer();
			return b;
		} catch {}
		return wasmMemory.buffer;
	}
	function updateMemoryViews() {
		if (HEAP8?.buffer?.resizable) return;
		var b = getMemoryBuffer();
		HEAP8 = new Int8Array(b);
		HEAP16 = new Int16Array(b);
		Module['HEAPU8'] = HEAPU8 = new Uint8Array(b);
		HEAPU16 = new Uint16Array(b);
		HEAP32 = new Int32Array(b);
		HEAPU32 = new Uint32Array(b);
		HEAPF32 = new Float32Array(b);
		HEAPF64 = new Float64Array(b);
		HEAP64 = new BigInt64Array(b);
		HEAPU64 = new BigUint64Array(b);
	}
	function preRun() {
		var preRun = Module['preRun'];
		if (preRun) {
			if (typeof preRun == 'function') preRun = [preRun];
			onPreRuns.push(...preRun);
		}
		callRuntimeCallbacks(onPreRuns);
	}
	function initRuntime() {
		runtimeInitialized = true;
		wasmExports['__wasm_call_ctors']();
	}
	function postRun() {
		var postRun = Module['postRun'];
		if (postRun) {
			if (typeof postRun == 'function') postRun = [postRun];
			onPostRuns.push(...postRun);
		}
		callRuntimeCallbacks(onPostRuns);
	}
	function abort(what) {
		Module['onAbort']?.(what);
		what = `Aborted(${what})`;
		err(what);
		ABORT = true;
		what += '. Build with -sASSERTIONS for more info.';
		var e = new WebAssembly.RuntimeError(what);
		throw e;
	}
	var wasmBinaryFile;
	function findWasmBinary() {
		if (Module['locateFile']) {
			return locateFile('ooz.wasm');
		}
		return new URL('ooz.wasm', import.meta.url).href;
	}
	function getBinarySync(file) {
		if (readBinary) {
			return readBinary(file);
		}
		throw 'both async and sync fetching of the wasm failed';
	}
	async function getWasmBinary(binaryFile) {
		if (!wasmBinary) {
			try {
				var response = await readAsync(binaryFile);
				return new Uint8Array(response);
			} catch {}
		}
		return getBinarySync(binaryFile);
	}
	async function instantiateArrayBuffer(binaryFile, imports) {
		try {
			var binary = await getWasmBinary(binaryFile);
			var instance = await WebAssembly.instantiate(binary, imports);
			return instance;
		} catch (reason) {
			err(`failed to asynchronously prepare wasm: ${reason}`);
			abort(reason);
		}
	}
	async function instantiateAsync(binary, binaryFile, imports) {
		if (!binary && !ENVIRONMENT_IS_NODE) {
			try {
				var response = fetch(binaryFile, { credentials: 'same-origin' });
				var instantiationResult = await WebAssembly.instantiateStreaming(response, imports);
				return instantiationResult;
			} catch (reason) {
				err(`wasm streaming compile failed: ${reason}`);
				err('falling back to ArrayBuffer instantiation');
			}
		}
		return instantiateArrayBuffer(binaryFile, imports);
	}
	function getWasmImports() {
		var imports = { env: wasmImports, wasi_snapshot_preview1: wasmImports };
		return imports;
	}
	async function createWasm() {
		function receiveInstance(instance) {
			wasmExports = instance.exports;
			wasmExports = applySignatureConversions(wasmExports);
			assignWasmExports(wasmExports);
			updateMemoryViews();
			return wasmExports;
		}
		function receiveInstantiationResult(result) {
			return receiveInstance(result['instance']);
		}
		var info = getWasmImports();
		var instantiateWasm = Module['instantiateWasm'];
		if (instantiateWasm) {
			return new Promise((resolve) => {
				instantiateWasm(info, (inst) => resolve(receiveInstance(inst)));
			});
		}
		wasmBinaryFile ??= findWasmBinary();
		var result = await instantiateAsync(wasmBinary, wasmBinaryFile, info);
		var exports = receiveInstantiationResult(result);
		return exports;
	}
	class ExitStatus {
		name = 'ExitStatus';
		constructor(status) {
			this.message = `Program terminated with exit(${status})`;
			this.status = status;
		}
	}
	var HEAP16;
	var HEAP32;
	var HEAP64;
	var HEAP8;
	var HEAPF32;
	var HEAPF64;
	var HEAPU16;
	var HEAPU32;
	var HEAPU64;
	var HEAPU8;
	var callRuntimeCallbacks = (callbacks) => {
		while (callbacks.length > 0) {
			callbacks.shift()(Module);
		}
	};
	var onPostRuns = [];
	var onPreRuns = [];
	var noExitRuntime = true;
	var stackRestore = (val) => __emscripten_stack_restore(val);
	var stackSave = () => _emscripten_stack_get_current();
	var INT53_MAX = 9007199254740992;
	var INT53_MIN = -9007199254740992;
	var UTF8Decoder = globalThis.TextDecoder && new TextDecoder();
	var findStringEnd = (heapOrArray, idx, maxBytesToRead, ignoreNul) => {
		var maxIdx = idx + maxBytesToRead;
		if (ignoreNul) return maxIdx;
		while (heapOrArray[idx] && !(idx >= maxIdx)) ++idx;
		return idx;
	};
	var UTF8ArrayToString = (heapOrArray, idx = 0, maxBytesToRead, ignoreNul) => {
		idx >>>= 0;
		var endPtr = findStringEnd(heapOrArray, idx, maxBytesToRead, ignoreNul);
		if (endPtr - idx > 16 && heapOrArray.buffer && UTF8Decoder) {
			return UTF8Decoder.decode(heapOrArray.subarray(idx, endPtr));
		}
		var str = '';
		while (idx < endPtr) {
			var u0 = heapOrArray[idx++];
			if (!(u0 & 128)) {
				str += String.fromCharCode(u0);
				continue;
			}
			var u1 = heapOrArray[idx++] & 63;
			if ((u0 & 224) == 192) {
				str += String.fromCharCode(((u0 & 31) << 6) | u1);
				continue;
			}
			var u2 = heapOrArray[idx++] & 63;
			if ((u0 & 240) == 224) {
				u0 = ((u0 & 15) << 12) | (u1 << 6) | u2;
			} else {
				u0 = ((u0 & 7) << 18) | (u1 << 12) | (u2 << 6) | (heapOrArray[idx++] & 63);
			}
			if (u0 < 65536) {
				str += String.fromCharCode(u0);
			} else {
				var ch = u0 - 65536;
				str += String.fromCharCode(55296 | (ch >> 10), 56320 | (ch & 1023));
			}
		}
		return str;
	};
	var UTF8ToString = (ptr, maxBytesToRead, ignoreNul) => {
		ptr >>>= 0;
		return ptr ? UTF8ArrayToString(HEAPU8, ptr, maxBytesToRead, ignoreNul) : '';
	};
	function ___assert_fail(condition, filename, line, func) {
		condition >>>= 0;
		filename >>>= 0;
		func >>>= 0;
		return abort(
			`Assertion failed: ${UTF8ToString(condition)}, at: ` +
				[
					filename ? UTF8ToString(filename) : 'unknown filename',
					line,
					func ? UTF8ToString(func) : 'unknown function'
				]
		);
	}
	class ExceptionInfo {
		constructor(excPtr) {
			this.excPtr = excPtr;
			this.ptr = excPtr - 24;
		}
		set_type(type) {
			HEAPU32[((this.ptr + 4) >>> 2) >>> 0] = type;
		}
		get_type() {
			return HEAPU32[((this.ptr + 4) >>> 2) >>> 0];
		}
		set_destructor(destructor) {
			HEAPU32[((this.ptr + 8) >>> 2) >>> 0] = destructor;
		}
		get_destructor() {
			return HEAPU32[((this.ptr + 8) >>> 2) >>> 0];
		}
		set_caught(caught) {
			caught = caught ? 1 : 0;
			HEAP8[(this.ptr + 12) >>> 0] = caught;
		}
		get_caught() {
			return HEAP8[(this.ptr + 12) >>> 0] != 0;
		}
		set_rethrown(rethrown) {
			rethrown = rethrown ? 1 : 0;
			HEAP8[(this.ptr + 13) >>> 0] = rethrown;
		}
		get_rethrown() {
			return HEAP8[(this.ptr + 13) >>> 0] != 0;
		}
		init(type, destructor) {
			this.set_adjusted_ptr(0);
			this.set_type(type);
			this.set_destructor(destructor);
		}
		set_adjusted_ptr(adjustedPtr) {
			HEAPU32[((this.ptr + 16) >>> 2) >>> 0] = adjustedPtr;
		}
		get_adjusted_ptr() {
			return HEAPU32[((this.ptr + 16) >>> 2) >>> 0];
		}
	}
	var uncaughtExceptionCount = 0;
	function ___cxa_throw(ptr, type, destructor) {
		ptr >>>= 0;
		type >>>= 0;
		destructor >>>= 0;
		var info = new ExceptionInfo(ptr);
		info.init(type, destructor);
		uncaughtExceptionCount++;
		abort();
	}
	var __abort_js = () => abort('');
	var getHeapMax = () => 4294901760;
	var alignMemory = (size, alignment) => Math.ceil(size / alignment) * alignment;
	var growMemory = (size) => {
		var oldHeapSize = wasmMemory.buffer.byteLength;
		var pages = ((size - oldHeapSize + 65535) / 65536) | 0;
		try {
			wasmMemory.grow(pages);
			updateMemoryViews();
			return 1;
		} catch (e) {}
	};
	function _emscripten_resize_heap(requestedSize) {
		requestedSize >>>= 0;
		var oldSize = HEAPU8.length;
		var maxHeapSize = getHeapMax();
		if (requestedSize > maxHeapSize) {
			return false;
		}
		for (var cutDown = 1; cutDown <= 4; cutDown *= 2) {
			var overGrownHeapSize = oldSize * (1 + 0.2 / cutDown);
			overGrownHeapSize = Math.min(overGrownHeapSize, requestedSize + 100663296);
			var newSize = Math.min(
				maxHeapSize,
				alignMemory(Math.max(requestedSize, overGrownHeapSize), 65536)
			);
			var replacement = growMemory(newSize);
			if (replacement) {
				return true;
			}
		}
		return false;
	}
	var printCharBuffers = [null, [], []];
	var printChar = (stream, curr) => {
		var buffer = printCharBuffers[stream];
		if (curr === 0 || curr === 10) {
			(stream === 1 ? out : err)(UTF8ArrayToString(buffer));
			buffer.length = 0;
		} else {
			buffer.push(curr);
		}
	};
	function _fd_write(fd, iov, iovcnt, pnum) {
		iov >>>= 0;
		iovcnt >>>= 0;
		pnum >>>= 0;
		var num = 0;
		for (var i = 0; i < iovcnt; i++) {
			var ptr = HEAPU32[(iov >>> 2) >>> 0];
			var len = HEAPU32[((iov + 4) >>> 2) >>> 0];
			iov += 8;
			for (var j = 0; j < len; j++) {
				printChar(fd, HEAPU8[(ptr + j) >>> 0]);
			}
			num += len;
		}
		HEAPU32[(pnum >>> 2) >>> 0] = num;
		return 0;
	}
	var getCFunc = (ident) => {
		var func = Module['_' + ident];
		return func;
	};
	var writeArrayToMemory = (array, buffer) => {
		HEAP8.set(array, buffer >>> 0);
	};
	var lengthBytesUTF8 = (str) => {
		var len = 0;
		for (var i = 0; i < str.length; ++i) {
			var c = str.charCodeAt(i);
			if (c <= 127) {
				len++;
			} else if (c <= 2047) {
				len += 2;
			} else if (c >= 55296 && c <= 57343) {
				len += 4;
				++i;
			} else {
				len += 3;
			}
		}
		return len;
	};
	var stringToUTF8Array = (str, heap, outIdx, maxBytesToWrite) => {
		outIdx >>>= 0;
		if (!(maxBytesToWrite > 0)) return 0;
		var startIdx = outIdx;
		var endIdx = outIdx + maxBytesToWrite - 1;
		for (var i = 0; i < str.length; ++i) {
			var u = str.codePointAt(i);
			if (u <= 127) {
				if (outIdx >= endIdx) break;
				heap[outIdx++ >>> 0] = u;
			} else if (u <= 2047) {
				if (outIdx + 1 >= endIdx) break;
				heap[outIdx++ >>> 0] = 192 | (u >> 6);
				heap[outIdx++ >>> 0] = 128 | (u & 63);
			} else if (u <= 65535) {
				if (outIdx + 2 >= endIdx) break;
				heap[outIdx++ >>> 0] = 224 | (u >> 12);
				heap[outIdx++ >>> 0] = 128 | ((u >> 6) & 63);
				heap[outIdx++ >>> 0] = 128 | (u & 63);
			} else {
				if (outIdx + 3 >= endIdx) break;
				heap[outIdx++ >>> 0] = 240 | (u >> 18);
				heap[outIdx++ >>> 0] = 128 | ((u >> 12) & 63);
				heap[outIdx++ >>> 0] = 128 | ((u >> 6) & 63);
				heap[outIdx++ >>> 0] = 128 | (u & 63);
				i++;
			}
		}
		heap[outIdx >>> 0] = 0;
		return outIdx - startIdx;
	};
	var stringToUTF8 = (str, outPtr, maxBytesToWrite) =>
		stringToUTF8Array(str, HEAPU8, outPtr, maxBytesToWrite);
	var stackAlloc = (sz) => __emscripten_stack_alloc(sz);
	var stringToUTF8OnStack = (str) => {
		var size = lengthBytesUTF8(str) + 1;
		var ret = stackAlloc(size);
		stringToUTF8(str, ret, size);
		return ret;
	};
	var ccall = (ident, returnType, argTypes, args, opts) => {
		var toC = {
			string: (str) => {
				var ret = 0;
				if (str !== null && str !== undefined && str !== 0) {
					ret = stringToUTF8OnStack(str);
				}
				return ret;
			},
			array: (arr) => {
				var ret = stackAlloc(arr.length);
				writeArrayToMemory(arr, ret);
				return ret;
			}
		};
		function convertReturnValue(ret) {
			if (returnType === 'string') {
				return UTF8ToString(ret);
			}
			if (returnType === 'pointer') return ret >>> 0;
			if (returnType === 'boolean') return Boolean(ret);
			return ret;
		}
		var func = getCFunc(ident);
		var cArgs = [];
		var stack = 0;
		if (args) {
			for (var i = 0; i < args.length; i++) {
				var converter = toC[argTypes[i]];
				if (converter) {
					if (stack === 0) stack = stackSave();
					cArgs[i] = converter(args[i]);
				} else {
					cArgs[i] = args[i];
				}
			}
		}
		var ret = func(...cArgs);
		function onDone(ret) {
			if (stack !== 0) stackRestore(stack);
			return convertReturnValue(ret);
		}
		ret = onDone(ret);
		return ret;
	};
	{
		if (Module['noExitRuntime']) noExitRuntime = Module['noExitRuntime'];
		if (Module['print']) out = Module['print'];
		if (Module['printErr']) err = Module['printErr'];
		if (Module['arguments']) programArgs = Module['arguments'];
		if (Module['thisProgram']) thisProgram = Module['thisProgram'];
		var preInit = Module['preInit'];
		if (preInit) {
			if (typeof preInit == 'function') Module['preInit'] = preInit = [preInit];
			while (preInit.length > 0) {
				preInit.shift()();
			}
		}
	}
	Module['ccall'] = ccall;
	var _malloc,
		_free,
		_Ooz_Decompress,
		_Ooz_Compress,
		__emscripten_stack_restore,
		__emscripten_stack_alloc,
		_emscripten_stack_get_current,
		memory,
		__indirect_function_table,
		wasmMemory;
	function assignWasmExports(wasmExports) {
		_malloc = Module['_malloc'] = wasmExports['malloc'];
		_free = Module['_free'] = wasmExports['free'];
		_Ooz_Decompress = Module['_Ooz_Decompress'] = wasmExports['Ooz_Decompress'];
		_Ooz_Compress = Module['_Ooz_Compress'] = wasmExports['Ooz_Compress'];
		__emscripten_stack_restore = wasmExports['_emscripten_stack_restore'];
		__emscripten_stack_alloc = wasmExports['_emscripten_stack_alloc'];
		_emscripten_stack_get_current = wasmExports['emscripten_stack_get_current'];
		memory = wasmMemory = wasmExports['memory'];
		__indirect_function_table = wasmExports['__indirect_function_table'];
	}
	var wasmImports = {
		__assert_fail: ___assert_fail,
		__cxa_throw: ___cxa_throw,
		_abort_js: __abort_js,
		emscripten_resize_heap: _emscripten_resize_heap,
		fd_write: _fd_write
	};
	function applySignatureConversions(wasmExports) {
		wasmExports = Object.assign({}, wasmExports);
		var makeWrapper_pp = (f) => (a0) => f(a0) >>> 0;
		var makeWrapper_p = (f) => () => f() >>> 0;
		wasmExports['malloc'] = makeWrapper_pp(wasmExports['malloc']);
		wasmExports['_emscripten_stack_alloc'] = makeWrapper_pp(wasmExports['_emscripten_stack_alloc']);
		wasmExports['emscripten_stack_get_current'] = makeWrapper_p(
			wasmExports['emscripten_stack_get_current']
		);
		return wasmExports;
	}
	async function run() {
		preRun();
		var setStatus = Module['setStatus'];
		if (setStatus) {
			setStatus('Running...');
			await new Promise((resolve) => setTimeout(resolve, 1));
			setTimeout(setStatus, 1, '');
		}
		if (ABORT) return;
		initRuntime();
		Module['onRuntimeInitialized']?.();
		postRun();
	}
	var wasmExports;
	wasmExports = await createWasm();
	await run();
	return Module;
}
export default Module;
