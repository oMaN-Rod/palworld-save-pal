import { assetLoader } from '$utils';
import { materialBlend, materialOpacity, materialTints, structureColors } from './mapColors.svelte';

const ASSET_DATA_PATH = '/src/lib/assets';

export const mapImg = {
	marker: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_00.webp`),
	star: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_01.webp`),
	box: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_02.webp`),
	pickaxe: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_03.webp`),
	sphere: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_04.webp`),
	food: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_05.webp`),
	boss: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_06.webp`),
	house: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_07.webp`),
	dungeon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_08.webp`),
	fruit: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_09.webp`),
	egg: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_10.webp`),
	player: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_11.webp`),
	chest: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_12.webp`),
	flower: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_13.webp`),
	oilPump: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_14.webp`),
	meteor: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_16.webp`),
	baseCamp: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_camp.webp`),
	dungeonLarge: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_dungeon.webp`),
	effigy: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_relic.webp`),
	fastTravel: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_fttower.webp`),
	watchTower: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_ftunlockmap.webp`),
	oilRig: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_oilrig.webp`),
	tower: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_tower.webp`),
	camp: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_camp.webp`),
	// The game's generic human silhouette. The 33 bounty targets each have their
	// own t_boss_npc_* portrait, but no one of them stands for the category.
	human: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/human.webp`),
	journal: assetLoader.loadImage(
		`${ASSET_DATA_PATH}/img/t_itemicon_consume_technologybook_g1.webp`
	),
	// The wall-mounted board, not the free-standing one: its post eats half the
	// icon box, leaving the sign itself unreadable at 24px.
	signboard: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_buildobject_wallsignboard.webp`),
	arrow: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_prt_compass_arrow.webp`)
};

/** Per-type relic icon, reusing the relic-stat art (`relic_<type>.webp`). */
export function relicTypeIcon(relicType: string): string {
	return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/relic_${relicType}.webp`);
}

function mix(a: string, b: string, t: number): string {
	const pa = [1, 3, 5].map((i) => parseInt(a.slice(i, i + 2), 16));
	const pb = [1, 3, 5].map((i) => parseInt(b.slice(i, i + 2), 16));
	const c = pa.map((v, i) => Math.round(v * (1 - t) + pb[i] * t));
	return `#${c.map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

export function structureFillColor(typeA: string, material?: string): string {
	const colors = structureColors();
	const base = colors[typeA] ?? colors.Other;
	const tint = material && material !== 'None' ? materialTints()[material] : undefined;
	return tint ? mix(base, tint, materialBlend()) : base;
}

export function structureOpacity(material?: string): number {
	return materialOpacity(material);
}
