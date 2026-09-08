import { SignalRoom } from './room';
import { validateUpgrade } from './policy';
import { handleTurnRequest, type RateLimit } from './turn';

export { SignalRoom };

export interface Env {
	SIGNAL_ROOM: DurableObjectNamespace<SignalRoom>;
	ASSETS: Fetcher;
	TURN_SECRET?: string;
	TURN_URLS?: string;
	TURN_TTL_SECONDS?: string;
	TURN_RATE?: RateLimit;
}

export default {
	async fetch(request: Request, env: Env): Promise<Response> {
		const url = new URL(request.url);
		if (url.pathname === '/signal/ws') {
			const v = validateUpgrade(url);
			if ('error' in v) return new Response(v.error, { status: 400 });
			const id = env.SIGNAL_ROOM.idFromName(v.room);
			return env.SIGNAL_ROOM.get(id).fetch(request);
		}
		if (url.pathname === '/signal/turn') return handleTurnRequest(request, env);
		return env.ASSETS.fetch(request);
	},
} satisfies ExportedHandler<Env>;
