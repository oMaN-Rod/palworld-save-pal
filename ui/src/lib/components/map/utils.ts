export type MapArea = 'MainMap' | 'Tree';

/** Bounds and textures from the game's DT_WorldMapUIData. Tree is listed first
 *  because it carries WorldMapPriority 1: where the rectangles overlap, it wins. */
export const MAP_AREAS: Record<MapArea, {
	texture: string;
	min: { x: number; y: number };
	max: { x: number; y: number };
}> = {
	Tree: {
		texture: 't_treemap.webp',
		min: { x: 347351.5, y: -818197.0 },
		max: { x: 689148.5, y: -476400.0 }
	},
	MainMap: {
		texture: 't_worldmap.webp',
		min: { x: -1099400.0, y: -724400.0 },
		max: { x: 349400.0, y: 724400.0 }
	}
};

export const MAP_AREA_ORDER: MapArea[] = ['MainMap', 'Tree'];

export const MAP_SIZE = 8192;

export const DEFAULT_MAP_AREA: MapArea = 'MainMap';

// The in-game coordinate readout (the numbers shown in the game's own UI) is a
// separate concern from pixel placement and keeps its original constants.
export const TRANSLATION_X = 123930.0;
export const TRANSLATION_Y = 157935.0;
export const SCALE = 459.0;

export function cmPerPx(area: MapArea): number {
	const { min, max } = MAP_AREAS[area];
	return (max.x - min.x) / MAP_SIZE;
}

/** Map horizontal axis is world +Y; map vertical axis is world -X. In OpenLayers'
 *  y-up pixel extent that flip cancels, leaving pixelY = (worldX - min.x) / cm. */
export function worldToPixel(worldX: number, worldY: number, area: MapArea): [number, number] {
	const { min } = MAP_AREAS[area];
	const cm = cmPerPx(area);
	return [(worldY - min.y) / cm, (worldX - min.x) / cm];
}

export function pixelToWorld(
	pixelX: number,
	pixelY: number,
	area: MapArea
): { worldX: number; worldY: number } {
	const { min } = MAP_AREAS[area];
	const cm = cmPerPx(area);
	return { worldX: pixelY * cm + min.x, worldY: pixelX * cm + min.y };
}

/** Which map a world position belongs to — the game's own rule, priority order. */
export function mapOf(worldX: number, worldY: number): MapArea | null {
	for (const area of Object.keys(MAP_AREAS) as MapArea[]) {
		const { min, max } = MAP_AREAS[area];
		if (worldX >= min.x && worldX <= max.x && worldY >= min.y && worldY <= max.y) {
			return area;
		}
	}
	return null;
}

export function worldToMap(worldX: number, worldY: number): { x: number; y: number } {
	return {
		x: Math.round((worldY - TRANSLATION_Y) / SCALE),
		y: Math.round((worldX + TRANSLATION_X) / SCALE) * -1
	};
}

export function mapToWorld(mapX: number, mapY: number): { x: number; y: number } {
	return {
		x: mapY * -1 * SCALE - TRANSLATION_X,
		y: mapX * SCALE + TRANSLATION_Y
	};
}

/** `BOSS_Horus_Water` -> `Horus_Water`, the key the pal data is stored under. */
export function bossPalKey(characterId: string | undefined): string | null {
	if (!characterId || characterId === 'None') return null;
	const key = characterId.replace(/^boss_/i, '');
	return key.length > 0 ? key : null;
}

/** Last-resort title for the human bosses, whose character_id is literally "None".
 *  `BOSS_Female_Soldier03` -> `Female Soldier 03`, `REGION_Oilrig_1` -> `Oilrig 1`. */
export function humanizeSpawnerId(spawnerId: string | undefined): string {
	if (!spawnerId) return 'Unknown';
	const name = spawnerId
		.replace(/^(BOSS|REGION)_/i, '')
		.replace(/_/g, ' ')
		.replace(/([a-z])([A-Z])/g, '$1 $2')
		.replace(/([A-Za-z])(\d)/g, '$1 $2')
		.replace(/\s+/g, ' ')
		.trim();
	return name.length > 0 ? name : 'Unknown';
}

export function pixelToGameCoords(
	pixelX: number,
	pixelY: number,
	area: MapArea
): { gameX: number; gameY: number } {
	const { worldX, worldY } = pixelToWorld(pixelX, pixelY, area);
	const mapCoords = worldToMap(worldX, worldY);
	// Tooltips all render `worldToMap(...).y * -1`; the readout must match them.
	return { gameX: mapCoords.x, gameY: mapCoords.y * -1 };
}
