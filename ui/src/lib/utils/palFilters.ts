import { elementsData } from '$lib/data';
import type { ElementType, Pal } from '$types';

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
