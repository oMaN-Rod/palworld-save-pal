// Archetype/material classification for the world-map structure registry.
// Name-driven rules first (most specific wins), then blueprint mesh-name
// hints, then a per-category default, then a plain box.

const NAME_RULES = [
	['trianglefoundation', 'triangleFoundation'],
	['trianglewallreverse', 'triangleWallReverse'],
	['trianglewall', 'triangleWall'],
	['triangleroof', 'gableRoof'],
	['pyramidroof', 'pyramidRoof'],
	['slopedroofcornerreverse', 'slopedRoofCornerReverse'],
	['slopedroofcorner', 'slopedRoofCorner'],
	['slantedroof', 'gableRoof'],
	['diagonalwall', 'diagonalWall'],
	['windowwall', 'wallWindow'],
	['doorwall', 'wallDoor'],
	['wallgate', 'wallGate'],
	['stair', 'stair'],
	['ladder', 'ladder'],
	['fence', 'fence'],
	['foundation', 'foundation'],
	['pillar', 'pillar'],
	['roof', 'roofFlat'],
	['wall', 'wall'],
	['lamp', 'lampPost'],
	['torch', 'torch'],
	['turret', 'turret'],
	['chest', 'chest'],
	['furnace', 'chimneyStack'],
	['planter', 'planter'],
	['farm', 'planter'],
	['tank', 'tank']
];

const MESH_RULES = [
	['turret', 'turret'],
	['stair', 'stair'],
	['roof', 'roofFlat'],
	['wall', 'wall'],
	['chest', 'chest'],
	['lamp', 'lampPost']
];

const CATEGORY_DEFAULT = {
	Storage: 'chest',
	Product: 'workstation',
	Defense: 'turret',
	Furniture: 'box',
	Food: 'planter',
	Light: 'lampPost',
	Infrastructure: 'box',
	Foundation: 'box',
	Pal: 'box',
	Other: 'box'
};

const norm = (s) => s.toLowerCase().replace(/[^a-z0-9]/g, '');

export function classifyArchetype(id, meshNames, typeA) {
	const key = norm(id);
	for (const [needle, archetype] of NAME_RULES) if (key.includes(needle)) return archetype;
	const meshKey = norm(meshNames.join(' '));
	for (const [needle, archetype] of MESH_RULES) if (meshKey.includes(needle)) return archetype;
	return CATEGORY_DEFAULT[typeA] ?? 'box';
}

const MATERIAL_PREFIX = [
	['glass', 'Glass'],
	['stone', 'Stone'],
	['palmetal', 'PalMetal'],
	['refinedmetal', 'PalMetal'],
	['metal', 'Metal'],
	['iron', 'Metal'],
	['ancient', 'Ancient'],
	['wooden', 'Wood'],
	['wood', 'Wood']
];

export function detectMaterial(id, materialType) {
	const key = norm(id);
	for (const [needle, mat] of MATERIAL_PREFIX) if (key.includes(needle)) return mat;
	if (materialType) {
		const tail = materialType.split('::').pop() ?? '';
		const t = norm(tail);
		for (const [needle, mat] of MATERIAL_PREFIX) if (t.includes(needle)) return mat;
	}
	return 'None';
}
