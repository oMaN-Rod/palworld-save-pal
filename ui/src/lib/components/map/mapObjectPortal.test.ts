import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import {
	createMapObjectPortalMesh,
	disposeMapObjectPortalMesh,
	FAST_TRAVEL_RADIUS_CM,
	fastTravelPortalColor,
	mapObjectPortalMatrix,
	palRingColor,
	portalRingColorExpression,
	RELIC_RADIUS_CM,
	relicPortalColor,
	type FastTravelState,
	type RelicState
} from './mapObjectPortal';
import { PORTAL_TAPER_RATIO } from './palPortal';

// Reads the ["match", ["get","state"], k1, v1, k2, v2, ..., fallback] shape
// built by portalRingColorExpression into a plain state -> hex lookup, the
// same way maplibre would resolve it for a feature with that state.
function ringArms(expr: ReturnType<typeof portalRingColorExpression>): Record<string, string> {
	const arr = expr as unknown as unknown[];
	const arms: Record<string, string> = {};
	for (let i = 2; i + 1 < arr.length; i += 2) {
		arms[arr[i] as string] = arr[i + 1] as string;
	}
	return arms;
}

describe('fastTravelPortalColor', () => {
	it('is neutral cyan when the state is unknown', () => {
		expect(fastTravelPortalColor('unknown').getHexString()).toBe('4fc3ff');
	});

	it('is amber when locked and cyan when unlocked', () => {
		expect(fastTravelPortalColor('locked').getHexString()).toBe('ffa726');
		expect(fastTravelPortalColor('unlocked').getHexString()).toBe('4fc3ff');
	});
});

describe('relicPortalColor', () => {
	it('is neutral cyan when the state is unknown', () => {
		expect(relicPortalColor('unknown').getHexString()).toBe('4fc3ff');
	});

	it('is amber when uncollected and green when collected', () => {
		expect(relicPortalColor('uncollected').getHexString()).toBe('ffa726');
		expect(relicPortalColor('collected').getHexString()).toBe('66bb6a');
	});
});

describe('palRingColor', () => {
	// Would break if the predator ring went back to reusing CORE_COLOR, or any
	// other blue, instead of this palette entry.
	it('is red for a predator, distinct from alpha and boss', () => {
		const predator = palRingColor('predator');
		expect(predator.getHexString()).not.toBe(palRingColor('alpha').getHexString());
		expect(predator.getHexString()).not.toBe(palRingColor('boss').getHexString());
		expect(predator.r).toBeGreaterThan(predator.g);
		expect(predator.r).toBeGreaterThan(predator.b);
	});

	// The boss-portal beam renders alpha and boss from one shared CORE_COLOR
	// uniform, so giving their rings independent colours would desync them.
	it('gives alpha and boss the same core color', () => {
		expect(palRingColor('alpha').getHexString()).toBe(palRingColor('boss').getHexString());
		expect(palRingColor('alpha').getHexString()).toBe('4fc3ff');
	});
});

describe('portalRingColorExpression', () => {
	// Guards the ring's ['match'] arms drifting from the beam's colour table.
	// Every state of both unions is checked, not just the ones that disagreed.
	const fastTravelStates: FastTravelState[] = ['unknown', 'locked', 'unlocked'];
	const relicStates: RelicState[] = ['unknown', 'uncollected', 'collected'];

	it('matches the beam color for every fast travel state', () => {
		const arms = ringArms(portalRingColorExpression('fastTravel'));
		for (const state of fastTravelStates) {
			expect(arms[state]).toBe(`#${fastTravelPortalColor(state).getHexString()}`);
		}
	});

	it('matches the beam color for every relic state', () => {
		const arms = ringArms(portalRingColorExpression('relic'));
		for (const state of relicStates) {
			expect(arms[state]).toBe(`#${relicPortalColor(state).getHexString()}`);
		}
	});

	it('names unknown as its own arm rather than relying on the fallback', () => {
		const fastTravelArms = ringArms(portalRingColorExpression('fastTravel'));
		const relicArms = ringArms(portalRingColorExpression('relic'));
		expect(fastTravelArms.unknown).toBe('#4fc3ff');
		expect(relicArms.unknown).toBe('#4fc3ff');
	});

	it('covers exactly the states of each union, no more and no fewer', () => {
		const fastTravelArms = ringArms(portalRingColorExpression('fastTravel'));
		const relicArms = ringArms(portalRingColorExpression('relic'));
		expect(Object.keys(fastTravelArms).sort()).toEqual([...fastTravelStates].sort());
		expect(Object.keys(relicArms).sort()).toEqual([...relicStates].sort());
	});
});

describe('mapObjectPortalMatrix', () => {
	it('produces a finite, invertible matrix', () => {
		const m = mapObjectPortalMatrix(-343155, 120000, 500, 'MainMap', 1e-9, 20);
		expect(m.elements.every(Number.isFinite)).toBe(true);
		expect(Math.abs(m.determinant())).toBeGreaterThan(0);
	});

	it('grows with the scale multiplier', () => {
		const small = mapObjectPortalMatrix(0, 0, 0, 'MainMap', 1e-9, 1);
		const large = mapObjectPortalMatrix(0, 0, 0, 'MainMap', 1e-9, 20);
		expect(Math.abs(large.determinant())).toBeGreaterThan(Math.abs(small.determinant()));
	});

	// Asserts that the translation moved, not which component: worldToPixel swaps
	// axes, so varying worldX moves latitude and leaves longitude untouched.
	it('places two different world positions apart', () => {
		const at = (wx: number, wy: number) =>
			new THREE.Vector3().setFromMatrixPosition(
				mapObjectPortalMatrix(wx, wy, 0, 'MainMap', 1e-9, 20)
			);
		expect(at(0, 0).distanceTo(at(100000, 0))).toBeGreaterThan(0);
		expect(at(0, 0).distanceTo(at(0, 100000))).toBeGreaterThan(0);
	});
});

describe('createMapObjectPortalMesh', () => {
	function baseRadiusOf(geometry: THREE.BufferGeometry): number {
		geometry.computeBoundingBox();
		const box = geometry.boundingBox!;
		return Math.max(box.max.x, box.max.y);
	}

	// The base radius is exactly the argument, not a fraction of it, so a ring
	// built from that same value can never disagree with the beam.
	it('builds its base (ground) radius from the radius argument, not a fixed fraction of it', () => {
		const relic = createMapObjectPortalMesh(1, RELIC_RADIUS_CM);
		expect(baseRadiusOf(relic.geometry)).toBeCloseTo(RELIC_RADIUS_CM, 4);
		disposeMapObjectPortalMesh(relic);

		const fastTravel = createMapObjectPortalMesh(1, FAST_TRAVEL_RADIUS_CM);
		expect(baseRadiusOf(fastTravel.geometry)).toBeCloseTo(FAST_TRAVEL_RADIUS_CM, 4);
		disposeMapObjectPortalMesh(fastTravel);
	});

	it('tapers the top to PORTAL_TAPER_RATIO of the same radius argument', () => {
		const mesh = createMapObjectPortalMesh(1, RELIC_RADIUS_CM);
		mesh.geometry.computeBoundingBox();
		const box = mesh.geometry.boundingBox!;
		expect(box.max.z - box.min.z).toBeGreaterThan(0);

		const topRadiusExpected = RELIC_RADIUS_CM * PORTAL_TAPER_RATIO;
		// The tapered end sits at max z after the rotate+translate, so read the
		// radius from the vertex closest to it.
		const position = mesh.geometry.getAttribute('position');
		let topRadius = 0;
		for (let i = 0; i < position.count; i++) {
			if (Math.abs(position.getZ(i) - box.max.z) < 1e-3) {
				topRadius = Math.max(topRadius, Math.hypot(position.getX(i), position.getY(i)));
			}
		}
		expect(topRadius).toBeCloseTo(topRadiusExpected, 2);
		disposeMapObjectPortalMesh(mesh);
	});
});
