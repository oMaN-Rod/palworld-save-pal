import { assetLoader } from '$utils';

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
	arrow: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_prt_compass_arrow.webp`)
};

/** Fixed rather than theme-derived: no theme palette carries ten distinguishable hues. */
export const STRUCTURE_COLORS: Record<string, string> = {
	Foundation: '#8d99ae',
	Furniture: '#c98b5e',
	Product: '#4d8fac',
	Storage: '#7a6a9c',
	Infrastructure: '#5c8a68',
	Pal: '#d4a13a',
	Light: '#d9cf6b',
	Defense: '#b5544a',
	Food: '#7fa84e',
	Other: '#6b7280'
};

/** Per-type relic icon, reusing the relic-stat art (`relic_<type>.webp`). */
export function relicTypeIcon(relicType: string): string {
	return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/relic_${relicType}.webp`);
}
