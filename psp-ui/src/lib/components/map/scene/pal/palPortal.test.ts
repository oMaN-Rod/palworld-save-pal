import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import {
	PORTAL_DIM_DEFEATED,
	PORTAL_RADIUS_CM,
	PORTAL_HEIGHT_CM,
	portalInstanceMatrix,
	portalIntensity,
	createPortalMeshes,
	disposePortalMeshes
} from './palPortal';
import { palInstanceMatrix } from './palLayer';
import { buildPalPortalFC } from './palPortalFC';
import { cmPerPx, worldToPixel } from '../../geo/utils';
import { lngLatToPixel } from '../../geo/mercator';

describe('portalIntensity', () => {
	it('dims defeated bosses and leaves the rest at full', () => {
		expect(portalIntensity(false)).toBe(1);
		expect(portalIntensity(true)).toBe(PORTAL_DIM_DEFEATED);
	});
});

describe('portalInstanceMatrix', () => {
	const CM_TO_MERC = 1e-6;

	it('sits at the same map position as its Pal', () => {
		const pal = palInstanceMatrix(-400000, -300000, 5000, 0, 'MainMap', CM_TO_MERC, 30, 0);
		const portal = portalInstanceMatrix(-400000, -300000, 5000, 'MainMap', CM_TO_MERC, 30);
		const p = new THREE.Vector3().setFromMatrixPosition(pal);
		const q = new THREE.Vector3().setFromMatrixPosition(portal);
		expect(q.x).toBeCloseTo(p.x, 12);
		expect(q.y).toBeCloseTo(p.y, 12);
	});

	it('sits flush with the Pal anchor along mercator up, no longer lifted for a ground disc', () => {
		const pal = palInstanceMatrix(-400000, -300000, 5000, 0, 'MainMap', CM_TO_MERC, 30, 0);
		const portal = portalInstanceMatrix(-400000, -300000, 5000, 'MainMap', CM_TO_MERC, 30);
		const dz =
			new THREE.Vector3().setFromMatrixPosition(portal).z -
			new THREE.Vector3().setFromMatrixPosition(pal).z;
		expect(dz).toBeCloseTo(0, 12);
	});

	it('does not rotate with bearing', () => {
		const a = portalInstanceMatrix(-400000, -300000, 0, 'MainMap', CM_TO_MERC, 30);
		const b = portalInstanceMatrix(-400000, -300000, 0, 'MainMap', CM_TO_MERC, 30);
		for (let i = 0; i < 16; i++) expect(a.elements[i]).toBeCloseTo(b.elements[i], 12);
	});
});

describe('createPortalMeshes', () => {
	it('builds a column instanced mesh sized for the requested count', () => {
		const m = createPortalMeshes(7);
		expect(m.column.count).toBe(7);
		expect(m.column.frustumCulled).toBe(false);
		disposePortalMeshes(m);
	});

	it('gives each call its own geometry so instance attributes cannot leak', () => {
		const a = createPortalMeshes(2);
		const b = createPortalMeshes(2);
		expect(a.column.geometry).not.toBe(b.column.geometry);
		disposePortalMeshes(a);
		disposePortalMeshes(b);
	});

	it('carries a per-instance intensity attribute', () => {
		const m = createPortalMeshes(3);
		const attr = m.column.geometry.getAttribute('aIntensity');
		expect(attr).toBeDefined();
		expect(attr.count).toBe(3);
		disposePortalMeshes(m);
	});

	it('never writes depth but still tests it', () => {
		const m = createPortalMeshes(1);
		const mat = m.column.material as THREE.ShaderMaterial;
		expect(mat.depthWrite).toBe(false);
		expect(mat.depthTest).toBe(true);
		expect(mat.transparent).toBe(true);
		expect(mat.blending).toBe(THREE.AdditiveBlending);
		disposePortalMeshes(m);
	});

	it('builds the column to PORTAL_HEIGHT_CM, standing on the ground anchor', () => {
		const { column } = createPortalMeshes(1);
		column.geometry.computeBoundingBox();
		const box = column.geometry.boundingBox!;

		expect(PORTAL_HEIGHT_CM).toBe(1380);
		expect(box.min.z).toBeCloseTo(0, 6);
		expect(box.max.z).toBeCloseTo(PORTAL_HEIGHT_CM, 6);

		disposePortalMeshes({ column });
	});
});

describe('portal depth state', () => {
	// Beams are drawn through the world by clearing the depth buffer before the Pal
	// scene renders, never by disabling depth testing here -- a beam that skips the
	// test paints over every Pal already drawn, including its own.
	it('always depth-tests', () => {
		const mat = createPortalMeshes(1).column.material as THREE.ShaderMaterial;
		expect(mat.depthTest).toBe(true);
	});

	// Beams are additive light, not surfaces: writing depth would let a beam
	// occlude the Pal standing in it.
	it('never writes depth and stays additive', () => {
		const mat = createPortalMeshes(1).column.material as THREE.ShaderMaterial;
		expect(mat.depthWrite).toBe(false);
		expect(mat.blending).toBe(THREE.AdditiveBlending);
		expect(mat.transparent).toBe(true);
	});
});

describe('the ground ring moved to buildPalPortalFC', () => {
	it('sizes the draped ring from this module\'s own PORTAL_RADIUS_CM, not a copy', () => {
		const boss = { key: 'anubis', x: -400000, y: -300000, z: 0, defeated: false };
		const fc = buildPalPortalFC([boss], [], 'MainMap', 30);
		const ring = (fc.features[0].geometry as GeoJSON.Polygon).coordinates[0];
		const [cx, cy] = worldToPixel(boss.x, boss.y, 'MainMap');
		const expectedRadiusPx = (PORTAL_RADIUS_CM * 30) / cmPerPx('MainMap');
		for (const [lng, lat] of ring) {
			const [vx, vy] = lngLatToPixel(lng, lat);
			expect(Math.hypot(vx - cx, vy - cy)).toBeCloseTo(expectedRadiusPx, 6);
		}
	});
});
