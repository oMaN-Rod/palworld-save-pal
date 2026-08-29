import { describe, expect, it } from 'vitest';
import * as THREE from 'three';
import { FIT_MARGIN, fitDistance, palBounds } from './palViewer';

const FOV = 35;

function meshAt(geometry: THREE.BufferGeometry, x = 0, y = 0, z = 0): THREE.Object3D {
	const root = new THREE.Group();
	const mesh = new THREE.Mesh(geometry);
	mesh.position.set(x, y, z);
	root.add(mesh);
	return root;
}

describe('fitDistance', () => {
	it('scales with the model, so a Jetdragon and a Lamball both fill the frame', () => {
		const near = fitDistance(100, FOV, 1.5);
		const far = fitDistance(400, FOV, 1.5);

		expect(far / near).toBeCloseTo(4, 6);
	});

	it('keeps the model inside the vertical field of view', () => {
		const radius = 250;

		const d = fitDistance(radius, FOV, 1.5);

		expect(Math.asin((radius * FIT_MARGIN) / d)).toBeCloseTo(((FOV / 2) * Math.PI) / 180, 6);
	});

	it('pulls back further when the panel is taller than it is wide', () => {
		const wide = fitDistance(250, FOV, 1.5);
		const narrow = fitDistance(250, FOV, 0.5);

		expect(narrow).toBeGreaterThan(wide);
	});

	it('is governed by the vertical field of view once the panel is wider than tall', () => {
		expect(fitDistance(250, FOV, 1)).toBeCloseTo(fitDistance(250, FOV, 3), 6);
	});

	it('moves closer as the lens widens', () => {
		expect(fitDistance(250, 60, 1.5)).toBeLessThan(fitDistance(250, 25, 1.5));
	});

	it('leaves the model some breathing room rather than touching the frame', () => {
		expect(FIT_MARGIN).toBeGreaterThan(1);
	});

	it('survives a degenerate bounding sphere', () => {
		expect(fitDistance(0, FOV, 1.5)).toBeGreaterThan(0);
	});
});

describe('palBounds', () => {
	it('finds the centre of a model the exporter left off the origin', () => {
		const root = meshAt(new THREE.BoxGeometry(2, 10, 2), 5, 3, -4);

		const { centre } = palBounds(root);

		expect([centre.x, centre.y, centre.z]).toEqual([5, 3, -4]);
	});

	it('accounts for the whole model, not just its first mesh', () => {
		const root = meshAt(new THREE.BoxGeometry(2, 2, 2), 0, 4, 0);
		const wing = new THREE.Mesh(new THREE.BoxGeometry(2, 2, 2));
		wing.position.set(0, -4, 0);
		root.add(wing);

		const { centre, radius } = palBounds(root);

		expect(centre.y).toBeCloseTo(0, 6);
		expect(radius).toBeGreaterThanOrEqual(5);
	});

	it('measures a radius that covers the model at every angle of the turntable', () => {
		const root = meshAt(new THREE.BoxGeometry(2, 10, 6), 0, 0, 0);

		const { radius } = palBounds(root);

		// Half-diagonal of the footprint (the deepest corner as it swings round)
		// against half the height.
		expect(radius).toBeCloseTo(Math.hypot(Math.hypot(1, 3), 5), 5);
	});

	it('does not inflate a round model to its bounding box corners', () => {
		const root = meshAt(new THREE.SphereGeometry(3, 32, 16));

		const { radius } = palBounds(root);

		expect(radius).toBeCloseTo(3, 1);
		expect(radius).toBeLessThan(Math.sqrt(27));
	});

	it('reports a usable radius for a model with no geometry at all', () => {
		const { centre, radius } = palBounds(new THREE.Group());

		expect(radius).toBeGreaterThan(0);
		expect(Number.isFinite(centre.y)).toBe(true);
	});
});
