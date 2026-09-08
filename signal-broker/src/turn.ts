export interface RateLimit {
	limit(opts: { key: string }): Promise<{ success: boolean }>;
}

export interface TurnEnv {
	TURN_SECRET?: string;
	TURN_URLS?: string;
	TURN_TTL_SECONDS?: string;
	TURN_RATE?: RateLimit;
}

export async function mintTurnCredential(
	secret: string,
	nowMs: number,
	ttlSeconds: number,
): Promise<{ username: string; credential: string }> {
	const expiry = Math.floor(nowMs / 1000) + ttlSeconds;
	const username = `${expiry}:psp`;
	const key = await crypto.subtle.importKey(
		'raw',
		new TextEncoder().encode(secret),
		{ name: 'HMAC', hash: 'SHA-1' },
		false,
		['sign'],
	);
	const sig = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(username));
	const credential = btoa(String.fromCharCode(...new Uint8Array(sig)));
	return { username, credential };
}

export async function handleTurnRequest(request: Request, env: TurnEnv): Promise<Response> {
	if (!env.TURN_SECRET) {
		return new Response(JSON.stringify({ error: 'unconfigured' }), {
			status: 503,
			headers: { 'content-type': 'application/json', 'cache-control': 'no-store' },
		});
	}

	if (env.TURN_RATE) {
		const key = request.headers.get('cf-connecting-ip') ?? 'unknown';
		const { success } = await env.TURN_RATE.limit({ key });
		if (!success) {
			return new Response(null, {
				status: 429,
				headers: { 'retry-after': '60', 'cache-control': 'no-store' },
			});
		}
	}

	const ttlSeconds = Number(env.TURN_TTL_SECONDS) || 3600;
	const { username, credential } = await mintTurnCredential(env.TURN_SECRET, Date.now(), ttlSeconds);
	const urls = (env.TURN_URLS ?? '').split(',').filter(Boolean);

	return new Response(JSON.stringify({ urls, username, credential, ttl_seconds: ttlSeconds }), {
		status: 200,
		headers: { 'content-type': 'application/json', 'cache-control': 'no-store' },
	});
}
