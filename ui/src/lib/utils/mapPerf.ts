// Lightweight perf marks for the map page. All logging is behind a single tag
// so the user can filter console to "[map-perf]" and see the whole timeline.
// Uses performance.now() relative to the first call.

let start = typeof performance !== 'undefined' ? performance.now() : 0;
let enabled = true;

export function mapPerfEnabled(next?: boolean): boolean {
	if (next !== undefined) enabled = next;
	return enabled;
}

export function mapPerfMark(stage: string, extra?: string): void {
	if (!enabled) return;
	try {
		const ms = typeof performance !== 'undefined' ? (performance.now() - start).toFixed(0) : '?';
		console.info(`[map-perf] +${ms}ms ${stage}${extra ? ` — ${extra}` : ''}`);
	} catch {}
}

export function mapPerfReset(): void {
	start = typeof performance !== 'undefined' ? performance.now() : 0;
}

export function mapPerfTime<T>(stage: string, fn: () => T): T {
	if (!enabled) return fn();
	const t0 = typeof performance !== 'undefined' ? performance.now() : 0;
	const result = fn();
	const dt = typeof performance !== 'undefined' ? (performance.now() - t0).toFixed(1) : '?';
	try {
		console.info(`[map-perf] ${stage}: ${dt}ms`);
	} catch {}
	return result;
}

export async function mapPerfTimeAsync<T>(stage: string, fn: () => Promise<T>): Promise<T> {
	if (!enabled) return fn();
	const t0 = typeof performance !== 'undefined' ? performance.now() : 0;
	try {
		return await fn();
	} finally {
		const dt = typeof performance !== 'undefined' ? (performance.now() - t0).toFixed(1) : '?';
		try {
			console.info(`[map-perf] ${stage}: ${dt}ms`);
		} catch {}
	}
}
