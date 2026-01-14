// Constants for coordinate conversion from the PalworldCoordinateConverter class
export const WORLD_MIN_X = -999940.0;
export const WORLD_MIN_Y = -738920.0;
export const WORLD_MAX_X = 447900.0;
export const WORLD_MAX_Y = 708920.0;

// Translation values from the coordinate converter
export const TRANSLATION_X = 123930.0;
export const TRANSLATION_Y = 157935.0;

// Scale factor for coordinate conversion
export const SCALE = 459.0;

// Map image size
export const MAP_SIZE = 8192;

// Game map coordinate ranges (based on the conversion of world coordinates)
export const GAME_MIN_X = -1951;
export const GAME_MIN_Y = -1893;
export const GAME_MAX_X = 1198;
export const GAME_MAX_Y = 1243;

// The game origin is located at (0, 0) in game coordinates
export const ORIGIN_GAME_X = 0;
export const ORIGIN_GAME_Y = 0;

// Calculate map width and height in game coordinates
export const MAP_WIDTH = GAME_MAX_X - GAME_MIN_X;
export const MAP_HEIGHT = GAME_MAX_Y - GAME_MIN_Y;

// Calculated transformation parameters for correct mapping
// These values map the game coordinates to the pixel display coordinates
export const TRANSFORM_A = MAP_SIZE / MAP_WIDTH; // Scale factor for X
export const TRANSFORM_B = 5075.45; // Offset for X (calculated to position origin correctly)
export const TRANSFORM_C = -MAP_SIZE / MAP_HEIGHT; // Scale factor for Y (negative for Y-axis inversion)
export const TRANSFORM_D = 4960.62; // Offset for Y (calculated to position origin correctly)

// Fixed: Y-coordinate is now inverted with * -1
export function worldToMap(worldX: number, worldY: number): { x: number; y: number } {
	const mapX = Math.round((worldY - TRANSLATION_Y) / SCALE);
	const mapY = Math.round((worldX + TRANSLATION_X) / SCALE) * -1;
	return { x: mapX, y: mapY };
}

// Since we've inverted Y in worldToMap, we need to invert it again here
export function mapToWorld(mapX: number, mapY: number): { x: number; y: number } {
	const worldX = mapY * -1 * SCALE - TRANSLATION_X; // Note the inversion of Y
	const worldY = mapX * SCALE + TRANSLATION_Y;
	return { x: worldX, y: worldY };
}

// Convert world coordinates to pixel coordinates for OpenLayers
// Returns [x, y] in pixel coordinates
export function worldToPixel(worldX: number, worldY: number): [number, number] {
	const mapCoords = worldToMap(worldX, worldY);
	// Transform game coordinates to pixel coordinates
	const pixelX = TRANSFORM_A * mapCoords.x + TRANSFORM_B;
	const pixelY = TRANSFORM_C * mapCoords.y + TRANSFORM_D;
	return [pixelX, pixelY];
}

// Convert pixel coordinates to world coordinates
export function pixelToWorld(pixelX: number, pixelY: number): { worldX: number; worldY: number } {
	// Convert from pixel to game coordinates
	// Reverse the transformation: x_game = (x_pixel - TRANSFORM_B) / TRANSFORM_A
	const gameX = (pixelX - TRANSFORM_B) / TRANSFORM_A;
	const gameY = (pixelY - TRANSFORM_D) / TRANSFORM_C;

	// Then convert from game coordinates to world coordinates
	const worldCoords = mapToWorld(gameX, gameY);
	return { worldX: worldCoords.x, worldY: worldCoords.y };
}

// Convert pixel coordinates to game (map) coordinates for display
export function pixelToGameCoords(
	pixelX: number,
	pixelY: number
): { gameX: number; gameY: number } {
	const gameX = (pixelX - TRANSFORM_B) / TRANSFORM_A;
	const gameY = ((pixelY - TRANSFORM_D) / TRANSFORM_C) * -1;
	return { gameX: Math.round(gameX), gameY: Math.round(gameY) };
}

// Convert map (game) coordinates to pixel coordinates
export function mapToPixel(mapX: number, mapY: number): [number, number] {
	const worldCoords = mapToWorld(mapX, mapY);
	return worldToPixel(worldCoords.x, worldCoords.y);
}
