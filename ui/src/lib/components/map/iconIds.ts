export const RELIC_TYPES = [
	'capture_power',
	'hunger_reduction',
	'swim_speed',
	'food_decay_reduction',
	'jump_power',
	'glider_speed',
	'climb_speed',
	'status_ailment_resist',
	'stamina_reduction',
	'sphere_homing',
	'exp_bonus',
	'rainbow_passive_rate',
	'move_speed'
] as const;

export const ICON_PLAYER = 'player';
export const ICON_BASE = 'baseCamp';
export const ICON_FAST_TRAVEL = 'fastTravel';
export const ICON_WATCHTOWER = 'watchTower';
export const ICON_DUNGEON = 'dungeon';
export const ICON_BOSS = 'boss';
export const ICON_ORIGIN = 'origin';
export const ICON_TOWER_BOSS = 'towerBoss';
export const ICON_EGG = 'egg';
export const ICON_CAMP = 'camp';
export const ICON_JOURNAL = 'journal';

export function relicIconId(relicType: string): string {
	return `relic:${relicType}`;
}

export function palIconId(pal: string, predator: boolean): string {
	return predator ? `pal:predator:${pal}` : `pal:alpha:${pal}`;
}
