import { ASSET_DATA_PATH } from '$lib/constants';
import { itemsData, palsData } from '$lib/data';
import { Rarity, type Item } from '$types';
import { staticIcons } from '$types/icons';
import { assetLoader } from '$utils';

export function getItemIcon(staticId: string): string | undefined {
	if (!staticId) return;
	const itemData = itemsData.getByKey(staticId);
	if (!itemData) {
		console.error(`Item data not found for static id: ${staticId}`);
		return;
	}
	if (!itemData.details.icon) {
		console.error(`Item icon not found for static id: ${staticId}`);
		return;
	}
	try {
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`);
	} catch (error) {
		console.error(`Failed to load image for static id: ${staticId}`);
		return;
	}
}

export function getItemTier(staticId: string, items: Item[]): string | undefined {
	if (!staticId) return;
	const itemData = items.find((item) => item.id === staticId);
	if (!itemData) {
		console.error(`Item data not found for static id: ${staticId}`);
		return;
	}
	return Rarity[itemData.details.rarity];
}

export function getBackgroundColor(staticId: string, items: Item[]): string {
	if (!staticId) return '';
	const itemData = items.find((item) => item.id === staticId);
	if (!itemData) {
		console.error(`Item data not found for static id: ${staticId}`);
		return '';
	}
	const tier = itemData.details.rarity;
	switch (tier) {
		case Rarity.Uncommon:
			return 'bg-linear-to-tl from-green-500/50';
		case Rarity.Rare:
			return 'bg-linear-to-tl from-blue-500/50';
		case Rarity.Epic:
			return 'bg-linear-to-tl from-purple-500/50';
		case Rarity.Legendary:
			return 'bg-linear-to-tl from-yellow-500/50';
		default:
			return '';
	}
}

export function getPalIcon(palId: string): string {
	if (!palId) return staticIcons.unknownIcon;
	const palData = palsData.getByKey(palId);
	return assetLoader.loadMenuImage(palId, palData?.is_pal ?? true);
}