// Per-instance transforms for the baked map-object meshes (fast travel statues,
// relic pedestals). The whole chain is the structure mesh path with the actor's
// yaw fixed at zero, since the save carries no rotation for these actors.
import { MercatorCoordinate } from 'maplibre-gl';
import * as THREE from 'three';
import { pixelToLngLat } from './mercator';
import { partLocalMatrix, type MeshPart } from './meshPlacement';
import { MESH_FLIP } from './structureLayer';
import { worldToPixel, type MapArea } from './utils';

export type MapObjectPart = MeshPart & { mesh: string };

export type MapObjectEntry = { parts: MapObjectPart[]; cullDistanceCm?: number };
export type MapObjectManifest = Record<string, MapObjectEntry>;

export function manifestParts(manifest: MapObjectManifest, actorClass: string): MapObjectPart[] {
	return manifest[actorClass]?.parts ?? [];
}

export function meshNames(manifest: MapObjectManifest): string[] {
	const names = new Set<string>();
	for (const entry of Object.values(manifest)) {
		for (const part of entry.parts) names.add(part.mesh);
	}
	return [...names];
}

export function mapObjectInstanceMatrix(
	part: MapObjectPart,
	worldX: number,
	worldY: number,
	worldZ: number,
	area: MapArea,
	cmToMerc: number
): THREE.Matrix4 {
	const [px, py] = worldToPixel(worldX, worldY, area);
	const [lng, lat] = pixelToLngLat(px, py);
	// Altitude goes through cmToMerc directly, not fromLngLat's altitude argument,
	// which would divide by this point's own latitude rather than the camera
	// centre's (see sceneryLayer.sceneryInstanceMatrix).
	const anchor = MercatorCoordinate.fromLngLat([lng, lat]);
	const anchorZ = worldZ * cmToMerc;
	const scale = new THREE.Matrix4().makeScale(cmToMerc, cmToMerc, cmToMerc);
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchorZ)
		.multiply(MESH_FLIP)
		.multiply(scale)
		.multiply(partLocalMatrix(part));
}
