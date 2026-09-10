const REFRESH_MARGIN_MS = 5 * 60 * 1000;
const FETCH_TIMEOUT_MS = 3_000;

interface TurnResponse {
	urls: string[];
	username: string;
	credential: string;
	ttl_seconds: number;
}

function resolveTurnUrl(): string {
	const envUrl = import.meta.env.VITE_SIGNAL_BROKER_URL as string | undefined;
	if (!envUrl) return '/signal/turn';
	const httpBase = envUrl.startsWith('wss://')
		? `https://${envUrl.slice('wss://'.length)}`
		: envUrl.startsWith('ws://')
			? `http://${envUrl.slice('ws://'.length)}`
			: envUrl;
	return `${httpBase.replace(/\/$/, '')}/signal/turn`;
}

export function createTurnProvider(
	fetchFn: typeof fetch = fetch,
	now: () => number = Date.now
): () => Promise<RTCIceServer[]> {
	let cached: { servers: RTCIceServer[]; expiresAtMs: number } | null = null;

	return async function getTurnServers(): Promise<RTCIceServer[]> {
		if (cached && cached.expiresAtMs - now() > REFRESH_MARGIN_MS) {
			return cached.servers;
		}

		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
		try {
			const response = await fetchFn(resolveTurnUrl(), { signal: controller.signal });
			if (!response.ok) return [];
			const body = (await response.json()) as TurnResponse;
			if (!Array.isArray(body.urls) || body.urls.length === 0) return [];
			const servers: RTCIceServer[] = [
				{ urls: body.urls, username: body.username, credential: body.credential }
			];
			cached = { servers, expiresAtMs: now() + body.ttl_seconds * 1000 };
			return servers;
		} catch {
			return [];
		} finally {
			clearTimeout(timer);
		}
	};
}
