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
	Foundation: '#9fa3a9', 
	Furniture: '#e07a5f',  
	Product: '#f4a261',    
	Storage: '#8d99ae',    
	Infrastructure: '#3d5a80', 
	Pal: '#00b4d8',        
	Light: '#f4d35e',      
	Defense: '#e63946',    
	Food: '#81b29a',       
	Other: '#6c757d'       
};
/** Per-type relic icon, reusing the relic-stat art (`relic_<type>.webp`). */
export function relicTypeIcon(relicType: string): string {
	return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/relic_${relicType}.webp`);
}

const MATERIAL_TINT: Record<string, string> = {
	Wood: '#8b5a2b',    
	Stone: '#9ba4b5',   
	Metal: '#393e46',   
	PalMetal: '#0077b6',
	Ancient: '#ffffff', 
	Glass: '#90e0ef'    
};

function mix(a: string, b: string, t: number): string {
	const pa = [1, 3, 5].map((i) => parseInt(a.slice(i, i + 2), 16));
	const pb = [1, 3, 5].map((i) => parseInt(b.slice(i, i + 2), 16));
	const c = pa.map((v, i) => Math.round(v * (1 - t) + pb[i] * t));
	return `#${c.map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

export function structureFillColor(typeA: string, material?: string): string {
	const base = STRUCTURE_COLORS[typeA] ?? STRUCTURE_COLORS.Other;
	const tint = material && material !== 'None' ? MATERIAL_TINT[material] : undefined;
	return tint ? mix(base, tint, 0.5) : base;
}
