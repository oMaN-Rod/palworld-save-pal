/** `MediaQuery` calls `matchMedia` once at construction, so install this before the module under test loads. */
export function installViewportStub(): {
	setViewport(width: number, options?: { coarse?: boolean; portrait?: boolean }): void;
} {
	const state = { width: 1440, coarse: false, portrait: true };
	// `MediaQuery` only re-reads `.matches` on a `change` event.
	const lists = new Set<{ query: string; listeners: Set<(event: Event) => void> }>();

	window.matchMedia = ((query: string) => {
		const listeners = new Set<(event: Event) => void>();
		lists.add({ query, listeners });

		return {
			get matches() {
				if (query.includes('pointer: coarse')) return state.coarse;
				if (query.includes('orientation: portrait')) return state.portrait;
				const max = /max-width:\s*(\d+)px/.exec(query);
				const min = /min-width:\s*(\d+)px/.exec(query);
				if (max) return state.width <= Number(max[1]);
				if (min) return state.width >= Number(min[1]);
				return false;
			},
			media: query,
			onchange: null,
			addListener: () => {},
			removeListener: () => {},
			addEventListener: (type: string, listener: (event: Event) => void) => {
				if (type === 'change') listeners.add(listener);
			},
			removeEventListener: (type: string, listener: (event: Event) => void) => {
				if (type === 'change') listeners.delete(listener);
			},
			dispatchEvent: () => false
		};
	}) as unknown as typeof window.matchMedia;

	return {
		setViewport(width, options = {}) {
			const { coarse = false, portrait = true } = options;
			state.width = width;
			state.coarse = coarse;
			state.portrait = portrait;
			for (const { listeners } of lists) {
				for (const listener of listeners) listener(new Event('change'));
			}
		}
	};
}
