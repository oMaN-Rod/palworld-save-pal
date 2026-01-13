import { assetLoader } from '$utils';
import type { Map } from 'ol';
import { Icon, Style } from 'ol/style';

const ASSET_DATA_PATH = '/src/lib/assets';

// Map image URLs - used directly in OpenLayers styles
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

export function createPalIconStyle(
	palImage: string,
	borderColor: string = '#ffffff',
	map?: Map | null
): Style {
	const size = 40;
	const borderWidth = 2;
	const innerRadius = size / 2 - borderWidth;

	// Create canvas for circular clipped image
	const canvas = document.createElement('canvas');
	canvas.width = size;
	canvas.height = size;
	const ctx = canvas.getContext('2d')!;

	// Load and draw the image clipped to circle
	const img = new Image();
	img.onload = () => {
		ctx.clearRect(0, 0, size, size);

		// Draw background circle first
		ctx.beginPath();
		ctx.arc(size / 2, size / 2, innerRadius, 0, Math.PI * 2);
		ctx.fill();

		// Draw the image clipped to circle on top
		ctx.save();
		ctx.beginPath();
		ctx.arc(size / 2, size / 2, innerRadius - 1, 0, Math.PI * 2);
		ctx.closePath();
		ctx.clip();

		// Scale and center the image to cover the circle
		const scale = Math.max(size / img.width, size / img.height);
		const w = img.width * scale;
		const h = img.height * scale;
		ctx.drawImage(img, (size - w) / 2, (size - h) / 2, w, h);
		ctx.restore();

		// Draw border on top of everything
		ctx.beginPath();
		ctx.arc(size / 2, size / 2, size / 2 - borderWidth / 2, 0, Math.PI * 2);
		ctx.strokeStyle = borderColor;
		ctx.lineWidth = borderWidth;
		ctx.stroke();

		// Trigger map re-render by changing the icon
		map?.render();
	};
	img.src = palImage;

	ctx.beginPath();
	ctx.arc(size / 2, size / 2, size / 2 - borderWidth / 2, 0, Math.PI * 2);
	ctx.strokeStyle = borderColor;
	ctx.lineWidth = borderWidth;
	ctx.stroke();

	return new Style({
		image: new Icon({
			img: canvas,
			size: [size, size],
			anchor: [0.5, 0.5],
			anchorXUnits: 'fraction',
			anchorYUnits: 'fraction'
		})
	});
}
