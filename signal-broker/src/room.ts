import { DurableObject } from 'cloudflare:workers';
import type { Env } from './index';
import { MAX_FRAME_BYTES, RATE, ROOM_TTL_MS, admitJoin, takeToken, validateUpgrade, type AdmissionRecord, type RoomKind } from './policy';

const ROLE_TAKEN_CLOSE = 4010;

export class SignalRoom extends DurableObject<Env> {
	constructor(ctx: DurableObjectState, env: Env) {
		super(ctx, env);
		// Protocol-level pings are answered by the runtime for free and never wake
		// the object, so there is deliberately NO application-level heartbeat here.
	}

	async fetch(request: Request): Promise<Response> {
		const v = validateUpgrade(new URL(request.url));
		if ('error' in v) return new Response(v.error, { status: 400 });

		const storedKind = await this.ctx.storage.get<RoomKind>('kind');
		if (storedKind !== undefined && storedKind !== v.kind) return new Response('kind mismatch', { status: 400 });

		if (this.ctx.getWebSockets(v.role).length > 0) {
			// A browser cannot read the status of a failed WebSocket upgrade — the
			// event it gets is an indistinguishable error — so a guest is turned
			// away over the one channel it can hear: the upgrade is accepted and
			// closed with the reason, and the socket is never tagged, so it does
			// not itself take the role. A desktop reads statuses fine and keeps
			// the 409 its reconnect ladder already treats as retryable.
			if (v.role !== 'guest') return new Response('role taken', { status: 409 });
			const refused = new WebSocketPair();
			refused[1].accept();
			refused[1].close(ROLE_TAKEN_CLOSE, 'role taken');
			return new Response(null, { status: 101, webSocket: refused[0] });
		}

		const now = Date.now();
		const record: AdmissionRecord =
			(await this.ctx.storage.get<AdmissionRecord>('admission')) ?? { joins: 0, windowStartMs: now };
		if (!admitJoin(record, now)) return new Response('too many attempts', { status: 429 });
		await this.ctx.storage.put('admission', record);
		if (storedKind === undefined) await this.ctx.storage.put('kind', v.kind);

		const pair = new WebSocketPair();
		this.ctx.acceptWebSocket(pair[1], [v.role]);
		pair[1].serializeAttachment({ bucket: { tokens: RATE.capacity, updatedMs: now } });

		if (v.kind === 'meet') {
			if (v.role === 'host') {
				await this.ctx.storage.deleteAlarm();
			} else if (this.ctx.getWebSockets('host').length === 0 && (await this.ctx.storage.getAlarm()) === null) {
				await this.ctx.storage.setAlarm(now + ROOM_TTL_MS);
			}
		} else if ((await this.ctx.storage.getAlarm()) === null) {
			// ONE non-recurring expiry alarm for the whole room, armed only if none is
			// pending. Alarm invocations bill as requests, so a recurring alarm would be
			// this design's dominant cost (288/day/room). Room lifetime past expiry is
			// bounded by the sockets themselves and by the desktop's own pairing window.
			await this.ctx.storage.setAlarm(now + ROOM_TTL_MS);
		}
		return new Response(null, { status: 101, webSocket: pair[0] });
	}

	async webSocketMessage(ws: WebSocket, msg: string | ArrayBuffer) {
		if (typeof msg !== 'string' || new TextEncoder().encode(msg).byteLength > MAX_FRAME_BYTES) {
			return ws.close(4009, 'frame too large');
		}
		const att = ws.deserializeAttachment();
		if (!takeToken(att.bucket, Date.now())) return ws.close(4008, 'rate limited');
		ws.serializeAttachment(att);
		const role = this.ctx.getTags(ws)[0];
		const other = role === 'host' ? 'guest' : 'host';
		for (const peer of this.ctx.getWebSockets(other)) peer.send(msg);
	}

	async webSocketClose(ws: WebSocket) {
		const role = this.ctx.getTags(ws)[0];
		if (role !== 'host' || this.ctx.getWebSockets('host').length > 0) return;
		if ((await this.ctx.storage.get<RoomKind>('kind')) !== 'meet') return;
		await this.ctx.storage.setAlarm(Date.now() + ROOM_TTL_MS);
	}

	// A socket that ends abnormally — a dropped connection, a crashed process —
	// is reported here INSTEAD of webSocketClose, so the room would otherwise
	// keep a departed host's storage alive until something else swept it.
	async webSocketError(ws: WebSocket) {
		return this.webSocketClose(ws);
	}

	async alarm() {
		for (const ws of this.ctx.getWebSockets()) ws.close(4001, 'pairing window expired');
		await this.ctx.storage.deleteAll();
	}
}
