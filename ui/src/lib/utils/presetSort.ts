export type PresetTypeKey =
	| 'pal_preset'
	| 'inventory'
	| 'passive_skills'
	| 'active_skills'
	| 'storage';

export type PresetSortMode = 'name' | 'custom';
export type SortDirection = 'asc' | 'desc';

export interface PresetSortConfig {
	mode: PresetSortMode;
	direction: SortDirection;
	customOrder: string[];
}

export const DEFAULT_CONFIG: PresetSortConfig = {
	mode: 'name',
	direction: 'asc',
	customOrder: []
};

export function orderPresets<T extends { id: string; name: string }>(
	list: T[],
	config: PresetSortConfig
): T[] {
	const byName = (a: T, b: T) => a.name.localeCompare(b.name);

	if (config.mode === 'name') {
		const sorted = [...list].sort(byName);
		return config.direction === 'desc' ? sorted.reverse() : sorted;
	}

	const orderIndex = new Map(config.customOrder.map((id, i) => [id, i]));
	const known = list
		.filter((p) => orderIndex.has(p.id))
		.sort((a, b) => orderIndex.get(a.id)! - orderIndex.get(b.id)!);
	const unknown = list.filter((p) => !orderIndex.has(p.id)).sort(byName);
	return [...known, ...unknown];
}
