export const CAPTURE_POWER = 'capture_power';

export type RelicPlayerView = {
	collected_effigies?: string[];
	collected_relics?: Record<string, string[]>;
	/** Unspent effigy counts per relic type; JSON `null` (not absent) on a
	 *  pre-1.0 save that carries no map. */
	relic_possess_num_map?: Record<string, number> | null;
	/** The CapturePower mirror of `relic_possess_num_map`. */
	effigy_possess_num?: number;
};

export type RelicRef = { guid: string; relic_type: string };

const same = (a: string, b: string) => a.toUpperCase() === b.toUpperCase();

/** Moves a possess-count entry by `delta`, floored at 0 -- the client mirror
 *  of the backend's `apply_relic_counters` net-delta rule. A spent relic cannot
 *  be un-spent, so un-collecting everything must stop at 0, not go negative.
 *  Only types the save already tracks move: the map is never created here. */
function bumpPossessCount(player: RelicPlayerView, relicType: string, delta: number): void {
	// `== null` covers both shapes: absent (synthetic test views) and the
	// JSON `null` the wire carries for a pre-1.0 save without a map.
	if (player.relic_possess_num_map == null || delta === 0) return;
	const current = player.relic_possess_num_map[relicType] ?? 0;
	player.relic_possess_num_map[relicType] = Math.max(0, current + delta);
}

// A pre-1.0 save has no by-type relic structure, so `collected_relics` arrives empty
// while `collected_effigies` still holds every collected Lifmunk Effigy. The write
// path rebuilds the effigy map wholesale from `collected_effigies`, so without this
// seed a single toggle would mirror a one-entry effigy list back and erase the rest.
export function relicsByType(player: RelicPlayerView): Record<string, string[]> {
	const byType = player.collected_relics ?? {};
	if (byType[CAPTURE_POWER]) return byType;
	return { ...byType, [CAPTURE_POWER]: [...(player.collected_effigies ?? [])] };
}

export function toggleRelic(player: RelicPlayerView, point: RelicRef): number {
	const byType = relicsByType(player);
	const collected = [...(byType[point.relic_type] ?? [])];
	const index = collected.findIndex((guid) => same(guid, point.guid));
	if (index >= 0) {
		collected.splice(index, 1);
	} else {
		collected.push(point.guid);
	}
	byType[point.relic_type] = collected;
	player.collected_relics = byType;
	// The unspent count follows the same ±1 the backend's counter write will
	// apply, so the Effigies panel and the round-tripped DTO stay in agreement.
	bumpPossessCount(player, point.relic_type, index >= 0 ? -1 : 1);
	if (point.relic_type !== CAPTURE_POWER) return 0;
	player.collected_effigies = [...collected];
	// Always carried by the real DTO (backend default 0); `?? 0` covers the
	// synthetic views the tests build.
	player.effigy_possess_num = Math.max(0, (player.effigy_possess_num ?? 0) + (index >= 0 ? -1 : 1));
	return index >= 0 ? -1 : 1;
}

export function collectRelics(
	player: RelicPlayerView,
	points: RelicRef[]
): { added: number; capturePowerAdded: number } {
	const byType = relicsByType(player);
	let added = 0;
	let capturePowerAdded = 0;
	const perTypeAdded = new Map<string, number>();
	for (const point of points) {
		const collected = byType[point.relic_type] ?? [];
		if (collected.some((guid) => same(guid, point.guid))) continue;
		byType[point.relic_type] = [...collected, point.guid];
		added++;
		perTypeAdded.set(point.relic_type, (perTypeAdded.get(point.relic_type) ?? 0) + 1);
		if (point.relic_type === CAPTURE_POWER) capturePowerAdded++;
	}
	if (added === 0) return { added: 0, capturePowerAdded: 0 };
	player.collected_relics = byType;
	player.collected_effigies = [...(byType[CAPTURE_POWER] ?? [])];
	for (const [relicType, count] of perTypeAdded) {
		bumpPossessCount(player, relicType, count);
	}
	if (capturePowerAdded > 0) {
		player.effigy_possess_num = Math.max(0, (player.effigy_possess_num ?? 0) + capturePowerAdded);
	}
	return { added, capturePowerAdded };
}
