import { elementsData } from '$lib/data';
import type { ElementType, Pal, PalData } from '$types';

export type PalCategory = 'normal' | 'quest' | 'boss' | 'special' | 'other';

const QUEST_MARKERS = ['QUEST_', '_QUEST_'];
const SPECIAL_PREFIXES = ['RAID_', 'PREDATOR_', 'SUMMON_', 'OILRIG'];

export function classifyPalCategory(key: string, pal: PalData): PalCategory {
	const upperKey = key.toUpperCase();

	if (QUEST_MARKERS.some((marker) => upperKey.includes(marker))) return 'quest';
	if (pal.is_boss || pal.is_tower_boss || upperKey.includes('GYM_') || upperKey.startsWith('BOSS_'))
		return 'boss';
	if (
		pal.is_raid_boss ||
		pal.predator ||
		SPECIAL_PREFIXES.some((prefix) => upperKey.includes(prefix))
	)
		return 'special';
	if (!pal.is_pal) return 'other';
	return 'normal';
}

export function palMatchesFilter(
	pal: Pal,
	palData: { is_pal: boolean; element_types: ElementType[] },
	selectedFilter: string
): boolean {
	if (Object.keys(elementsData.elements).includes(selectedFilter)) {
		return palData.element_types
			.map((element: ElementType) => element.toString()!.toLowerCase())
			.includes(selectedFilter.toLowerCase());
	}

	const characterId = pal.character_id.toLowerCase();
	switch (selectedFilter) {
		case 'alpha':
			return pal.is_boss;
		case 'lucky':
			return pal.is_lucky;
		case 'awakened':
			return pal.is_awakened;
		case 'imported':
			return pal.is_imported;
		case 'human':
			return !palData.is_pal;
		case 'predator':
			return characterId.includes('predator_');
		case 'oilrig':
			return characterId.includes('_oilrig');
		case 'summon':
			return characterId.includes('summon_');
		default:
			return true;
	}
}
