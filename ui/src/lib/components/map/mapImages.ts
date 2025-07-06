import { assetLoader } from '$utils';
import L from 'leaflet';

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
	fastTravel: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_fttower.webp`),
	oilRig: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_oilrig.webp`),
	tower: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_tower.webp`),
	arrow: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_prt_compass_arrow.webp`),
	worldMap: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_worldmap.webp`, 'webp')
};

export const mapIcons = {
	marker: L.icon({
		iconUrl: mapImg.marker,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	star: L.icon({
		iconUrl: mapImg.star,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	box: L.icon({
		iconUrl: mapImg.box,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	pickaxe: L.icon({
		iconUrl: mapImg.pickaxe,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	sphere: L.icon({
		iconUrl: mapImg.sphere,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	food: L.icon({
		iconUrl: mapImg.food,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	boss: L.icon({
		iconUrl: mapImg.boss,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	house: L.icon({
		iconUrl: mapImg.house,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	dungeon: L.icon({
		iconUrl: mapImg.dungeon,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	fruit: L.icon({
		iconUrl: mapImg.fruit,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	egg: L.icon({
		iconUrl: mapImg.egg,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	player: L.icon({
		iconUrl: mapImg.player,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	chest: L.icon({
		iconUrl: mapImg.chest,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	flower: L.icon({
		iconUrl: mapImg.flower,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	oilPump: L.icon({
		iconUrl: mapImg.oilPump,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	meteor: L.icon({
		iconUrl: mapImg.meteor,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	baseCamp: L.icon({
		iconUrl: mapImg.baseCamp,
		iconSize: [40, 40],
		iconAnchor: [20, 20],
		popupAnchor: [0, -20]
	}),
	dungeonLarge: L.icon({
		iconUrl: mapImg.dungeonLarge,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	fastTravel: L.icon({
		iconUrl: mapImg.fastTravel,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	oilRig: L.icon({
		iconUrl: mapImg.oilRig,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	tower: L.icon({
		iconUrl: mapImg.tower,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	}),
	arrow: L.icon({
		iconUrl: mapImg.arrow,
		iconSize: [48, 48],
		iconAnchor: [24, 24],
		popupAnchor: [0, -16]
	})
};
