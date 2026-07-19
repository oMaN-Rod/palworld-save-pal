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

// Shift the selected ids one slot toward `direction`, each hopping over the
// adjacent unselected neighbor. A selected block stops at the top/bottom;
// non-contiguous selections move independently. Returns a new array.
export function moveIds(
	orderedIds: string[],
	selectedIds: Set<string>,
	direction: 'up' | 'down'
): string[] {
	const ids = [...orderedIds];
	if (direction === 'up') {
		for (let i = 1; i < ids.length; i++) {
			if (selectedIds.has(ids[i]) && !selectedIds.has(ids[i - 1])) {
				[ids[i - 1], ids[i]] = [ids[i], ids[i - 1]];
			}
		}
	} else {
		for (let i = ids.length - 2; i >= 0; i--) {
			if (selectedIds.has(ids[i]) && !selectedIds.has(ids[i + 1])) {
				[ids[i + 1], ids[i]] = [ids[i], ids[i + 1]];
			}
		}
	}
	return ids;
}
