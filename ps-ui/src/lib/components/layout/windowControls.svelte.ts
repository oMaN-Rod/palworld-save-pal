export type TauriWindow = {
	minimize(): Promise<void>;
	toggleMaximize(): Promise<void>;
	close(): Promise<void>;
	isMaximized(): Promise<boolean>;
	onResized(handler: () => void): Promise<() => void>;
};

type TauriGlobal = { window?: { getCurrentWindow?: () => TauriWindow } };

// Resolved lazily on every call — never at module scope, which would break
// prerendering.
export function currentWindow(): TauriWindow | undefined {
	if (typeof window === 'undefined') return undefined;
	return (window as Window & { __TAURI__?: TauriGlobal }).__TAURI__?.window?.getCurrentWindow?.();
}

export type WindowControls = {
	readonly maximized: boolean;
	minimize(): void;
	toggleMaximize(): void;
	close(): void;
	sync(): Promise<void>;
	watch(): () => void;
};

export function createWindowControls(resolve: () => TauriWindow | undefined = currentWindow) {
	let maximized = $state(false);

	async function sync(): Promise<void> {
		const win = resolve();
		if (!win) return;
		maximized = await win.isMaximized();
	}

	return {
		get maximized() {
			return maximized;
		},
		minimize: () => void resolve()?.minimize(),
		toggleMaximize: () => void resolve()?.toggleMaximize(),
		close: () => void resolve()?.close(),
		sync,
		// onResized resolves asynchronously; without the disposed flag a teardown
		// that runs first would leak the listener.
		watch(): () => void {
			let unlisten: (() => void) | undefined;
			let disposed = false;

			void sync();
			void resolve()
				?.onResized(() => void sync())
				.then((stop) => {
					if (disposed) stop();
					else unlisten = stop;
				});

			return () => {
				disposed = true;
				unlisten?.();
			};
		}
	} satisfies WindowControls;
}
