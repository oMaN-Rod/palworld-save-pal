export const CAPTURE_POWER = 'capture_power';

export type RelicPlayerView = {
	collected_effigies?: string[];
	collected_relics?: Record<string, string[]>;
};

export type RelicRef = { guid: string; relic_type: string };

const same = (a: string, b: string) => a.toUpperCase() === b.toUpperCase();

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
	if (point.relic_type !== CAPTURE_POWER) return 0;
	player.collected_effigies = [...collected];
	return index >= 0 ? -1 : 1;
}

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
