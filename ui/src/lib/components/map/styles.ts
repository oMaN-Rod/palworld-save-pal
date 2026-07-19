import type { Base } from '$types';
import { assetLoader } from '$utils';
import type { Map } from 'ol';
import type { FeatureLike } from 'ol/Feature';
import { Fill, Icon, Stroke, Style } from 'ol/style';
import CircleStyle from 'ol/style/Circle';
import { createIconStyle, createStyle } from 'svelte-openlayers';
import { cmPerPx, type MapArea } from './utils';
import compass from '$lib/assets/img/compass.webp';
import { isWatchtower } from './fastTravel';

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
	effigy: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_relic.webp`),
	fastTravel: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_fttower.webp`),
	watchTower: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_ftunlockmap.webp`),
	oilRig: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_oilrig.webp`),
	tower: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_compass_tower.webp`),
	arrow: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_prt_compass_arrow.webp`),
	maps: {
		MainMap: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_worldmap.webp`, 'webp'),
		Tree: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_treemap.webp`, 'webp')
	} as Record<MapArea, string>
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

// Icon styles
export const playerIconStyle = createIconStyle({
	src: mapImg.player,
	scale: 1,
	anchor: [0.5, 0.5],
	anchorXUnits: 'fraction',
	anchorYUnits: 'fraction'
});

export const baseIconStyle = (area: MapArea) => (feature: FeatureLike, resolution: number) => {
	const props = feature.getProperties();
	const base = props.data as Base;
	const areaRange = base.area_range || 3500;
	const screenRadius = areaRange / cmPerPx(area) / resolution;
	return [
		createIconStyle({
			src: mapImg.baseCamp,
			scale: 0.83,
			anchor: [0.5, 0.5],
			anchorXUnits: 'fraction',
			anchorYUnits: 'fraction'
		}),
		new Style({
			image: new CircleStyle({
				radius: screenRadius,
				stroke: new Stroke({ color: 'rgba(0, 0, 255, 1)', width: 2, lineDash: [4, 8] }),
				fill: new Fill({ color: 'rgba(0, 0, 255, 0.1)' })
			})
		})
	];
};

export const fastTravelIconStyle = createIconStyle({
	src: mapImg.fastTravel,
	scale: 0.75,
	anchor: [0.5, 0.5],
	anchorXUnits: 'fraction',
	anchorYUnits: 'fraction',
	opacity: 1
});

export const fastTravelLockedIconStyle = createIconStyle({
	src: mapImg.fastTravel,
	scale: 0.75,
	anchor: [0.5, 0.5],
	anchorXUnits: 'fraction',
	anchorYUnits: 'fraction',
	opacity: 0.6
});

export const watchTowerIconStyle = createIconStyle({
	src: mapImg.watchTower,
	scale: 0.6,
	anchor: [0.5, 0.5],
	anchorXUnits: 'fraction',
	anchorYUnits: 'fraction',
	opacity: 1
});

export const watchTowerLockedIconStyle = createIconStyle({
	src: mapImg.watchTower,
	scale: 0.6,
	anchor: [0.5, 0.5],
	anchorXUnits: 'fraction',
	anchorYUnits: 'fraction',
	opacity: 0.6
});

export const fastTravelStyle = (feature: FeatureLike) => {
	const data = feature.get('data');
	const locked = data?.unlocked === false;
	if (isWatchtower(data ?? {})) {
		return locked ? watchTowerLockedIconStyle : watchTowerIconStyle;
	}
	return locked ? fastTravelLockedIconStyle : fastTravelIconStyle;
};

/** Per-type relic icon, reusing the relic-stat art (`relic_<type>.webp`). */
export function relicTypeIcon(relicType: string): string {
	return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/relic_${relicType}.webp`);
}

const relicStyleCache: Record<string, Style> = {};

function relicIconStyle(relicType: string, collected: boolean): Style {
	const cacheKey = `${relicType}:${collected}`;
	const cached = relicStyleCache[cacheKey];
	if (cached) return cached;
	const style = new Style({
		image: new Icon({
			src: relicTypeIcon(relicType),
			scale: 0.5,
			anchor: [0.5, 0.5],
			opacity: collected ? 1 : 0.6
		})
	});
	relicStyleCache[cacheKey] = style;
	return style;
}

export const relicStyle = (feature: FeatureLike) => {
	const data = feature.get('data');
	return relicIconStyle(data?.relic_type ?? 'capture_power', data?.unlocked !== false);
};

export const dungeonIconStyle = createIconStyle({
	src: mapImg.dungeon,
	scale: 1,
	anchor: [0.5, 0.5],
	anchorXUnits: 'fraction',
	anchorYUnits: 'fraction'
});

export const bossIconStyle = createIconStyle({
	src: mapImg.boss,
	scale: 1,
	anchor: [0.5, 0.5],
	anchorXUnits: 'fraction',
	anchorYUnits: 'fraction',
	opacity: 1
});

export const bossDefeatedIconStyle = createIconStyle({
	src: mapImg.boss,
	scale: 1,
	anchor: [0.5, 0.5],
	anchorXUnits: 'fraction',
	anchorYUnits: 'fraction',
	opacity: 0.6
});

export const bossStyle = (feature: FeatureLike) => {
	const data = feature.get('data');
	return data?.defeated === true ? bossDefeatedIconStyle : bossIconStyle;
};

export const originIconStyle = createStyle({
	image: {
		src: compass,
		scale: 1,
		anchor: [0.5, 0.5],
		anchorXUnits: 'fraction',
		anchorYUnits: 'fraction'
	}
});
export const originLineStyle = createStyle({
	stroke: { color: '#ffffff', width: 0.5, lineDash: [4, 8] }
});
