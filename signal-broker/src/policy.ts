export const MAX_FRAME_BYTES = 4096;
export const ROOM_TTL_MS = 5 * 60_000;
export const RATE = { capacity: 20, refillPerSec: 2 };
export const ADMISSION = { maxJoinsPerRoom: 12, windowMs: 5 * 60_000 };

export type Bucket = { tokens: number; updatedMs: number };
export type AdmissionRecord = { joins: number; windowStartMs: number };

const ROOM_RE = /^[0-9a-f]{32}$/;

export type RoomKind = 'pair' | 'meet';

export function validateUpgrade(url: URL): { room: string; role: 'host' | 'guest'; kind: RoomKind } | { error: string } {
	const room = url.searchParams.get('room') ?? '';
	const role = url.searchParams.get('role');
	const kind = url.searchParams.get('kind') ?? 'pair';
	if (!ROOM_RE.test(room)) return { error: 'invalid room' };
	if (role !== 'host' && role !== 'guest') return { error: 'invalid role' };
	if (kind !== 'pair' && kind !== 'meet') return { error: 'invalid kind' };
	return { room, role, kind };
}

export function takeToken(b: Bucket, nowMs: number): boolean {
	const elapsedSec = Math.max(0, (nowMs - b.updatedMs) / 1000);
	b.tokens = Math.min(RATE.capacity, b.tokens + elapsedSec * RATE.refillPerSec);
	b.updatedMs = nowMs;
	if (b.tokens < 1) return false;
	b.tokens -= 1;
	return true;
}

export function admitJoin(record: AdmissionRecord, nowMs: number): boolean {
	if (nowMs - record.windowStartMs > ADMISSION.windowMs) {
		record.joins = 0;
		record.windowStartMs = nowMs;
	}
	if (record.joins >= ADMISSION.maxJoinsPerRoom) return false;
	record.joins += 1;
	return true;
}
