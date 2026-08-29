import type { MapUnlockPoint, RelicPoint } from '$types';
import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { buildFastTravelRingFC, buildRelicRingFC } from './mapObjectItems';
import {
	createMapObjectPortalMesh,
	disposeMapObjectPortalMesh,
	FAST_TRAVEL_RADIUS_CM,
	RELIC_RADIUS_CM
} from './mapObjectPortal';
import { lngLatToPixel } from '../../geo/mercator';
import type { PalBoss, PalPredator } from '../pal/palLayer';
import { createPortalMeshes, disposePortalMeshes, PORTAL_RADIUS_CM } from '../pal/palPortal';
import { buildPalPortalFC } from '../pal/palPortalFC';
import type { MapArea } from '../../geo/utils';
import { cmPerPx, worldToPixel } from '../../geo/utils';

const AREA: MapArea = 'MainMap';
const WORLD_X = -400000;
const WORLD_Y = -300000;
const SCALES = [0.5, 1, 3, 20];

const FT_POINT: MapUnlockPoint = {
	guid: 'ft1',
	x: WORLD_X,
	y: WORLD_Y,
	z: 0,
	localized_name: 'Statue'
};
const RELIC_POINT: RelicPoint = {
	guid: 'r1',
	x: WORLD_X,
	y: WORLD_Y,
	z: 0,
	localized_name: 'Relic',
	relic_type: 'jump_power'
};

function ringRadiusCm(fc: GeoJSON.FeatureCollection): number {
	const ring = (fc.features[0].geometry as GeoJSON.Polygon).coordinates[0];
	const [cx, cy] = worldToPixel(WORLD_X, WORLD_Y, AREA);
	const radiiPx = ring.map(([lng, lat]) => {
		const [vx, vy] = lngLatToPixel(lng, lat);
		return Math.hypot(vx - cx, vy - cy);
	});
	const avgPx = radiiPx.reduce((a, b) => a + b, 0) / radiiPx.length;
	return avgPx * cmPerPx(AREA);
}

// Beam geometry is built unscaled -- the size slider drives the per-instance
// transform -- so its widest circle, the ground-level base rather than the
// tapered top, is what must match the ring.
function beamBaseRadiusCm(geometry: THREE.BufferGeometry): number {
	geometry.computeBoundingBox();
	const box = geometry.boundingBox!;
	return Math.max(box.max.x, box.max.y, Math.abs(box.min.x), Math.abs(box.min.y));
}

function expectClose(beamCm: number, ringCm: number) {
	expect(Math.abs(beamCm - ringCm)).toBeLessThan(Math.max(ringCm * 1e-3, 1e-6));
}

// For every marker type, the beam's light and the ring beneath it must be the
// same size and concentric at every value of that marker's size slider. These
// hold only if both were built from one radius per marker type: reverting either
// mesh builder to a fraction of PORTAL_RADIUS_CM breaks them.
describe('marker ring/light radius parity', () => {
	describe.each(SCALES)('at scale %s', (scale) => {
		it('boss (and alpha): beam base radius equals the ring radius', () => {
			const boss: PalBoss = { key: 'anubis', x: WORLD_X, y: WORLD_Y, z: 0, defeated: false };
			const ring = ringRadiusCm(buildPalPortalFC([boss], [], AREA, scale));

			const { column } = createPortalMeshes(1);
			const beam = beamBaseRadiusCm(column.geometry) * scale;
			disposePortalMeshes({ column });

			expectClose(beam, ring);
		});

		it('predator: beam base radius equals the ring radius', () => {
			const predator: PalPredator = { key: 'sifudog', x: WORLD_X, y: WORLD_Y, z: 0 };
			const ring = ringRadiusCm(buildPalPortalFC([], [predator], AREA, scale));

			const mesh = createMapObjectPortalMesh(1, PORTAL_RADIUS_CM);
			const beam = beamBaseRadiusCm(mesh.geometry) * scale;
			disposeMapObjectPortalMesh(mesh);

			expectClose(beam, ring);
		});

		it('relic: beam base radius equals the ring radius', () => {
			const ring = ringRadiusCm(buildRelicRingFC([RELIC_POINT], AREA, scale));

			const mesh = createMapObjectPortalMesh(1, RELIC_RADIUS_CM);
			const beam = beamBaseRadiusCm(mesh.geometry) * scale;
			disposeMapObjectPortalMesh(mesh);

			expectClose(beam, ring);
		});

		it('fast travel: beam base radius equals the ring radius', () => {
			const ring = ringRadiusCm(buildFastTravelRingFC([FT_POINT], AREA, scale, scale * 3));

			const mesh = createMapObjectPortalMesh(1, FAST_TRAVEL_RADIUS_CM);
			const beam = beamBaseRadiusCm(mesh.geometry) * scale;
			disposeMapObjectPortalMesh(mesh);

			expectClose(beam, ring);
		});
	});
});
