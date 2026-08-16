import compass from '$lib/assets/img/compass.webp';
import {
	ICON_BASE,
	ICON_BOSS,
	ICON_CAMP,
	ICON_DUNGEON,
	ICON_EGG,
	ICON_FAST_TRAVEL,
	ICON_JOURNAL,
	ICON_ORIGIN,
	ICON_PLAYER,
	ICON_TOWER_BOSS,
	ICON_WATCHTOWER,
	RELIC_TYPES,
	relicIconId
} from './iconIds';
import { mapImg, relicTypeIcon } from './styles';

export const PAL_BORDER_ALPHA = '#ffffff';
export const PAL_BORDER_PREDATOR = '#ef4444';

export function staticIconUrls(): Record<string, string> {
	const urls: Record<string, string> = {
		[ICON_PLAYER]: mapImg.player,
		[ICON_BASE]: mapImg.baseCamp,
		[ICON_FAST_TRAVEL]: mapImg.fastTravel,
		[ICON_WATCHTOWER]: mapImg.watchTower,
		[ICON_DUNGEON]: mapImg.dungeon,
		[ICON_BOSS]: mapImg.boss,
		[ICON_ORIGIN]: compass,
		[ICON_TOWER_BOSS]: mapImg.tower,
		[ICON_EGG]: mapImg.egg,
		[ICON_CAMP]: mapImg.camp,
		[ICON_JOURNAL]: mapImg.journal
	};
	for (const type of RELIC_TYPES) {
		urls[relicIconId(type)] = relicTypeIcon(type);
	}
	return urls;
}

export function renderPalIcon(palImageUrl: string, borderColor: string): Promise<ImageData> {
	const size = 40;
	const borderWidth = 2;
	const innerRadius = size / 2 - borderWidth;

	return new Promise((resolve, reject) => {
		const canvas = document.createElement('canvas');
		canvas.width = size;
		canvas.height = size;
		const ctx = canvas.getContext('2d');
		if (!ctx) {
			reject(new Error('2d context unavailable'));
			return;
		}

		const img = new Image();
		img.onload = () => {
			ctx.clearRect(0, 0, size, size);

			ctx.save();
			ctx.beginPath();
			ctx.arc(size / 2, size / 2, innerRadius - 1, 0, Math.PI * 2);
			ctx.closePath();
			ctx.clip();

			const scale = Math.max(size / img.width, size / img.height);
			const w = img.width * scale;
			const h = img.height * scale;
			ctx.drawImage(img, (size - w) / 2, (size - h) / 2, w, h);
			ctx.restore();

			ctx.beginPath();
			ctx.arc(size / 2, size / 2, size / 2 - borderWidth / 2, 0, Math.PI * 2);
			ctx.strokeStyle = borderColor;
			ctx.lineWidth = borderWidth;
			ctx.stroke();

			resolve(ctx.getImageData(0, 0, size, size));
		};
		img.onerror = () => reject(new Error(`failed to load ${palImageUrl}`));
		img.src = palImageUrl;
	});
}
