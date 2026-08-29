import { Rarity } from '$types/game';

function getComputedColor(varName: string) {
	// Read during component init, which also happens server-side while
	// prerendering. There is no computed style there; the client recomputes on
	// hydration.
	if (typeof document === 'undefined' || typeof getComputedStyle === 'undefined') {
		return '';
	}
	const computedStyle = getComputedStyle(document.body);
	const value = computedStyle.getPropertyValue(varName).trim();
	return value;
}

export function getComputedColorHex(varName: string): string {
	const color = getComputedColor(varName);
	return rgbToHex(color) ?? '#000000';
}

export function rgbToHex(rgbString: string | undefined) {
	if (!rgbString) return null;
	const rgb = rgbString.split(' ');

	const r = parseInt(rgb[0]);
	const g = parseInt(rgb[1]);
	const b = parseInt(rgb[2]);

	const hexR = componentToHex(r);
	const hexG = componentToHex(g);
	const hexB = componentToHex(b);

	return `#${hexR}${hexG}${hexB}`;
}

export function hexToRGB(hex: string) {
	hex = hex.replace('#', '');

	const r = parseInt(hex.substring(0, 2), 16) / 255;
	const g = parseInt(hex.substring(2, 4), 16) / 255;
	const b = parseInt(hex.substring(4, 6), 16) / 255;

	return { r, g, b };
}

export function calculateFilters(hex: string) {
	const rgb = hexToRGB(hex);

	const matrix = [
		rgb.r,
		0,
		0,
		0,
		0,
		0,
		rgb.g,
		0,
		0,
		0,
		0,
		0,
		rgb.b,
		0,
		0,
		0,
		0,
		0,
		1,
		0
	];

	return `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'><filter id='colorize'><feColorMatrix type='matrix' values='${matrix.join(' ')}'/></filter></svg>#colorize")`;
}

function componentToHex(c: number) {
	const hex = c.toString(16);
	return hex.length === 1 ? '0' + hex : hex;
}

export function skillBorderClass(rank: number) {
	switch (rank) {
		case 1:
			return 'border-l-surface-600';
		case 2:
		case 3:
			return 'border-l-[#fcdf19]';
		case 4:
		case 5:
			return 'border-l-[#68ffd8]';
		default:
			return 'border-l-[#FF0000]';
	}
}

export function skillOpacity(rank: number) {
	switch (rank) {
		case 1:
		case 2:
		case 3:
		case 4:
		case 5:
			return 'opacity-25';
	}
	return 'opacity-15';
}

export function skillFilter(rank: number) {
	switch (rank) {
		case 1:
			return '';
		case 2:
		case 3:
			return calculateFilters('#fcdf19');
		case 4:
		case 5:
			return calculateFilters('#68ffd8');
		default:
			return calculateFilters('#FF0000');
	}
}
export function rarityGradientClass(rarity: number | undefined) {
	switch (rarity) {
		case Rarity.Uncommon:
			return 'bg-linear-to-tl from-green-200/50 to-green-800/75';
		case Rarity.Rare:
			return 'bg-linear-to-tl from-blue-200/50 to-blue-800/75';
		case Rarity.Epic:
			return 'bg-linear-to-tl from-purple-200/50 to-purple-800/75';
		case Rarity.Legendary:
			return 'bg-linear-to-tl from-yellow-200/50 to-yellow-700/75';
		default:
			return '';
	}
}

export function rarityAccentClass(rarity: number | undefined) {
	switch (rarity) {
		case Rarity.Uncommon:
			return 'text-green-300 border-green-500';
		case Rarity.Rare:
			return 'text-blue-300 border-blue-500';
		case Rarity.Epic:
			return 'text-purple-300 border-purple-500';
		case Rarity.Legendary:
			return 'text-yellow-300 border-yellow-500';
		default:
			return '';
	}
}

export function raritySolidClass(rarity: number | undefined) {
	switch (rarity) {
		case Rarity.Uncommon:
			return 'bg-green-800 text-green-300 border-green-500';
		case Rarity.Rare:
			return 'bg-blue-800 text-blue-300 border-blue-500';
		case Rarity.Epic:
			return 'bg-purple-800 text-purple-300 border-purple-500';
		case Rarity.Legendary:
			return 'bg-yellow-800 text-yellow-300 border-yellow-500';
		default:
			return 'bg-surface-900 text-surface-300 border-surface-500';
	}
}
