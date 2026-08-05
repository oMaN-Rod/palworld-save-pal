import { persistedState } from 'svelte-persisted-state';
import {
	DEFAULT_CONFIG,
	orderPresets,
	type PresetSortConfig,
	type PresetSortMode,
	type PresetTypeKey,
	type SortDirection
} from '$utils/presetSort';

export type { PresetSortConfig, PresetSortMode, PresetTypeKey, SortDirection } from '$utils/presetSort';

const DEFAULTS: Record<PresetTypeKey, PresetSortConfig> = {
	pal_preset: { ...DEFAULT_CONFIG },
	inventory: { ...DEFAULT_CONFIG },
	passive_skills: { ...DEFAULT_CONFIG },
	active_skills: { ...DEFAULT_CONFIG },
	storage: { ...DEFAULT_CONFIG }
};

export const presetSort = persistedState<Record<PresetTypeKey, PresetSortConfig>>(
	'psp-preset-sort',
	DEFAULTS
);

export function getConfig(type: PresetTypeKey): PresetSortConfig {
	return presetSort.current[type] ?? DEFAULT_CONFIG;
}

export function setMode(type: PresetTypeKey, mode: PresetSortMode): void {
	if (getConfig(type).mode === mode) return;
	presetSort.current = { ...presetSort.current, [type]: { ...getConfig(type), mode } };
}

export function setDirection(type: PresetTypeKey, direction: SortDirection): void {
	if (getConfig(type).direction === direction) return;
	presetSort.current = { ...presetSort.current, [type]: { ...getConfig(type), direction } };
}

export function setCustomOrder(type: PresetTypeKey, ids: string[]): void {
	const current = getConfig(type).customOrder;
	if (current.length === ids.length && current.every((id, i) => id === ids[i])) return;
	presetSort.current = { ...presetSort.current, [type]: { ...getConfig(type), customOrder: ids } };
}

export function sortPresets<T extends { id: string; name: string }>(
	list: T[],
	type: PresetTypeKey
): T[] {
	return orderPresets(list, getConfig(type));
}
