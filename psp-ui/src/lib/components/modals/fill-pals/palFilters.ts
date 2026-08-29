import { palsData } from '$lib/data';
import type { PalData } from '$types';

const SPECIAL_CASES = ['PREDATOR_', 'RAID_', 'GYM_', 'SUMMON_', '_OILRIG'];

function sortByDeckIndex(a: [string, PalData], b: [string, PalData]): number {
	const indexA = a[1]?.pal_deck_index ?? Infinity;
	const indexB = b[1]?.pal_deck_index ?? Infinity;
	return indexA - indexB;
}

function filterEnabled(): [string, PalData][] {
	return Object.entries(palsData.pals).filter((p) => !p[1].disabled) as [string, PalData][];
}

export function getHumanPals(): [string, PalData][] {
	return filterEnabled()
		.filter((p) => !p[1].is_pal)
		.sort(sortByDeckIndex);
}

export function getNormalPals(): [string, PalData][] {
	return filterEnabled()
		.filter(
			(p) =>
				!SPECIAL_CASES.some((substring) => p[0].toUpperCase().includes(substring)) &&
				p[1].is_pal
		)
		.sort(sortByDeckIndex);
}

export function getPredatorPals(): [string, PalData][] {
	return filterEnabled()
		.filter((p) => p[0].toUpperCase().includes('PREDATOR_'))
		.sort(sortByDeckIndex);
}

export function getRaidPals(): [string, PalData][] {
	return filterEnabled()
		.filter((p) => p[0].toUpperCase().includes('RAID_'))
		.sort(sortByDeckIndex);
}

export function getBossPals(): [string, PalData][] {
	return filterEnabled()
		.filter((p) => p[0].toUpperCase().includes('GYM_'))
		.sort(sortByDeckIndex);
}

export function getSummonPals(): [string, PalData][] {
	return filterEnabled()
		.filter((p) => p[0].toUpperCase().includes('SUMMON_'))
		.sort(sortByDeckIndex);
}

export function getOilRigPals(): [string, PalData][] {
	return filterEnabled()
		.filter((p) => p[0].toUpperCase().includes('OILRIG_'))
		.sort(sortByDeckIndex);
}