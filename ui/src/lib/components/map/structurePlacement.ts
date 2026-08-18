// Per-structure placement for the 3D custom layer. Anchor lng/lat come from the
// same world->pixel->lngLat transform the flat extrusion uses; altitude is the
// structure's own world Z (cm) so meshes ride the DEM surface. The caller applies
// verticalScale (cm->MapLibre metres) exactly as <Terrain exaggeration> does.
import type { BaseStructure, Footprint } from '$types';
import { pixelToLngLat } from './mercator';
import type { MapArea } from './utils';
import { worldToPixel } from './utils';

export type StructurePlacement = {
	lng: number;
	lat: number;
	altitudeCm: number;
	footprintCm: { sx: number; sy: number; sz: number };
	yaw: number;
};

export type StructureAnchor = { lng: number; lat: number; altitudeCm: number; yaw: number };

// The raw actor transform, with no footprint offset applied. Real game meshes
// already sit correctly relative to the actor origin, unlike the collision-box
// proxy geometry that structurePlacement's fp.ox/oy/oz centers.
export function structureAnchor(s: BaseStructure, area: MapArea): StructureAnchor {
	const [px, py] = worldToPixel(s.x, s.y, area);
	const [lng, lat] = pixelToLngLat(px, py);
	return { lng, lat, altitudeCm: s.z, yaw: s.yaw };
}

export function structurePlacement(
	s: BaseStructure,
	fp: Footprint,
	area: MapArea,
	_verticalScale: number
): StructurePlacement {
	const cos = Math.cos(s.yaw);
	const sin = Math.sin(s.yaw);
	const cx = s.x + fp.ox * s.scale_x * cos - fp.oy * s.scale_y * sin;
	const cy = s.y + fp.ox * s.scale_x * sin + fp.oy * s.scale_y * cos;
	const [px, py] = worldToPixel(cx, cy, area);
	const [lng, lat] = pixelToLngLat(px, py);
	return {
		lng,
		lat,
		altitudeCm: s.z + fp.oz * s.scale_z,
		footprintCm: { sx: fp.sx * s.scale_x, sy: fp.sy * s.scale_y, sz: fp.sz * s.scale_z },
		yaw: s.yaw
	};
}
