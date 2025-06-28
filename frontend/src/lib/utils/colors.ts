function getComputedColor(varName: string) {
	const computedStyle = getComputedStyle(document.body);
	const value = computedStyle.getPropertyValue(varName).trim();
	return value;
}

export function getComputedColorHex(varName: string): string {
	const color = getComputedColor(varName);
	const hex = rgbToHex(color) as string;
	return hex;
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
	// Remove # if present
	hex = hex.replace('#', '');

	const r = parseInt(hex.substring(0, 2), 16) / 255;
	const g = parseInt(hex.substring(2, 4), 16) / 255;
	const b = parseInt(hex.substring(4, 6), 16) / 255;

	return { r, g, b };
}

export function calculateFilters(hex: string) {
	const rgb = hexToRGB(hex);

	// Matrix for color transformation
	const matrix = [
		rgb.r,
		0,
		0,
		0,
		0, // Red
		0,
		rgb.g,
		0,
		0,
		0, // Green
		0,
		0,
		rgb.b,
		0,
		0, // Blue
		0,
		0,
		0,
		1,
		0 // Alpha
	];

	return `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'><filter id='colorize'><feColorMatrix type='matrix' values='${matrix.join(' ')}'/></filter></svg>#colorize")`;
}

function componentToHex(c: number) {
	const hex = c.toString(16);
	return hex.length === 1 ? '0' + hex : hex;
}
