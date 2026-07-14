/** CapturePower relics ARE the Lifmunk Effigies. */
export const CAPTURE_POWER = 'capture_power';

export type RelicPlayerView = {
	collected_effigies?: string[];
	collected_relics?: Record<string, string[]>;
};

export type RelicRef = { guid: string; relic_type: string };

const same = (a: string, b: string) => a.toUpperCase() === b.toUpperCase();

/**
 * The by-type relic view of a player, with `capture_power` seeded from the flat
 * `collected_effigies` list when the by-type entry is absent.
 *
 * WHY: a pre-1.0 save has no by-type relic structure at all, so `collected_relics`
 * arrives as an empty map `{}` while `collected_effigies` still holds every Lifmunk
 * Effigy the player collected. The write path rebuilds the effigy map wholesale from
 * `collected_effigies`, so without this seed a single toggle would mirror an
 * effigy list of one back onto the save and erase all the others. Seeding makes the
 * read (pins drawn as collected) and write (effigy list) views share one baseline.
 * On a 1.0 save the two already agree, so this is a no-op.
 */
export function relicsByType(player: RelicPlayerView): Record<string, string[]> {
	const byType = player.collected_relics ?? {};
	if (byType[CAPTURE_POWER]) return byType;
	return { ...byType, [CAPTURE_POWER]: [...(player.collected_effigies ?? [])] };
}

/**
 * Toggle one relic on the player. Returns the delta to apply to the `Relic` item
 * count (non-zero only for capture_power, which is the item-backed effigy).
 */
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
	if (point.relic_type !== CAPTURE_POWER) return 0;
	player.collected_effigies = [...collected];
	return index >= 0 ? -1 : 1;
}

/**
 * Collect every relic in `points` that the player does not already have.
 * `capturePowerAdded` counts what was ACTUALLY newly added, so the `Relic` item count
 * moves by the true delta rather than by the count of visible pins.
 */
export function collectRelics(
	player: RelicPlayerView,
	points: RelicRef[]
): { added: number; capturePowerAdded: number } {
	const byType = relicsByType(player);
	let added = 0;
	let capturePowerAdded = 0;
	for (const point of points) {
		const collected = byType[point.relic_type] ?? [];
		if (collected.some((guid) => same(guid, point.guid))) continue;
		byType[point.relic_type] = [...collected, point.guid];
		added++;
		if (point.relic_type === CAPTURE_POWER) capturePowerAdded++;
	}
	if (added === 0) return { added: 0, capturePowerAdded: 0 };
	player.collected_relics = byType;
	player.collected_effigies = [...(byType[CAPTURE_POWER] ?? [])];
	return { added, capturePowerAdded };
}
