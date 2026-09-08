export type PipCorner = 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';

export interface PipRect {
	x: number;
	y: number;
	width: number;
	height: number;
}

export const PIP_PANEL_MARGIN = 12;
export const PIP_PANEL_DEFAULT_WIDTH = 320;
export const PIP_PANEL_DEFAULT_HEIGHT = 240;
export const PIP_PANEL_MIN_WIDTH = 240;
export const PIP_PANEL_MIN_HEIGHT = 160;

export function nearestCorner(
	rect: PipRect,
	viewportWidth: number,
	viewportHeight: number
): PipCorner {
	const centerX = rect.x + rect.width / 2;
	const centerY = rect.y + rect.height / 2;
	const right = centerX > viewportWidth / 2;
	const bottom = centerY > viewportHeight / 2;
	return `${bottom ? 'bottom' : 'top'}-${right ? 'right' : 'left'}`;
}

export function cornerOrigin(
	corner: PipCorner,
	width: number,
	height: number,
	viewportWidth: number,
	viewportHeight: number,
	margin = PIP_PANEL_MARGIN
): { x: number; y: number } {
	const x = corner.endsWith('right') ? viewportWidth - width - margin : margin;
	const y = corner.startsWith('bottom') ? viewportHeight - height - margin : margin;
	return { x, y };
}

export function clampToViewport(
	rect: PipRect,
	viewportWidth: number,
	viewportHeight: number,
	minWidth = 0,
	minHeight = 0
): PipRect {
	const width = Math.min(Math.max(rect.width, minWidth), viewportWidth);
	const height = Math.min(Math.max(rect.height, minHeight), viewportHeight);
	const x = Math.min(Math.max(rect.x, 0), Math.max(0, viewportWidth - width));
	const y = Math.min(Math.max(rect.y, 0), Math.max(0, viewportHeight - height));
	if (x === rect.x && y === rect.y && width === rect.width && height === rect.height) {
		return rect;
	}
	return { x, y, width, height };
}
