import type { LiveActorJson } from '$lib/signal/session.svelte';
import type { MapArea } from '../geo/utils';
import { inArea } from './features';

export type FollowVerdict =
	| { kind: 'ease'; x: number; y: number }
	| { kind: 'waiting' }
	| { kind: 'drop' };

export const FOLLOW_GRACE_MS = 10_000;

export function followVerdict(
	actors: LiveActorJson[],
	followedId: string,
	area: MapArea,
	nowMs: number,
	lastSeenMs: number | null
): { verdict: FollowVerdict; lastSeenMs: number | null } {
	const actor = actors.find((a) => a.id === followedId);
	if (
		actor &&
		Number.isFinite(actor.x) &&
		Number.isFinite(actor.y) &&
		inArea(actor.x, actor.y, area)
	) {
		if (actor.kind === 'palbox') return { verdict: { kind: 'drop' }, lastSeenMs };
		return { verdict: { kind: 'ease', x: actor.x, y: actor.y }, lastSeenMs: nowMs };
	}

	if (lastSeenMs === null) return { verdict: { kind: 'waiting' }, lastSeenMs: nowMs };
	if (nowMs - lastSeenMs < FOLLOW_GRACE_MS) return { verdict: { kind: 'waiting' }, lastSeenMs };
	return { verdict: { kind: 'drop' }, lastSeenMs };
}
