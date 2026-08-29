import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { buildArchetypeGeometry } from './proxyGeometry';

const bbox = (g: THREE.BufferGeometry) => {
	g.computeBoundingBox();
	return g.boundingBox!;
};

describe('buildArchetypeGeometry', () => {
	it('box spans the given extents and is bottom-anchored at y=0', () => {
		const g = buildArchetypeGeometry('box', 100, 200, 300);
		const b = bbox(g);
		expect(b.min.y).toBeCloseTo(0, 5);
		expect(b.max.y).toBeCloseTo(300, 5);
		expect(b.max.x - b.min.x).toBeCloseTo(100, 5);
		expect(b.max.z - b.min.z).toBeCloseTo(200, 5);
	});

	it('foundation is top-anchored at y=0 (extends downward)', () => {
		const g = buildArchetypeGeometry('foundation', 400, 400, 40);
		const b = bbox(g);
		expect(b.max.y).toBeCloseTo(0, 5);
		expect(b.min.y).toBeCloseTo(-40, 5);
	});

	it('pyramidRoof apex reaches the full height', () => {
		const g = buildArchetypeGeometry('pyramidRoof', 400, 400, 150);
		const b = bbox(g);
		expect(b.max.y).toBeCloseTo(150, 3);
		expect(b.min.y).toBeCloseTo(0, 3);
	});

	it('caches identical requests (same reference)', () => {
		const a = buildArchetypeGeometry('box', 100, 100, 100);
		const b = buildArchetypeGeometry('box', 100, 100, 100);
		expect(a).toBe(b);
	});

	it('unknown archetype falls back to a box', () => {
		const g = buildArchetypeGeometry('nonsense', 100, 100, 100);
		const b = bbox(g);
		expect(b.max.y).toBeCloseTo(100, 5);
	});
});

describe('furniture and ladder archetypes', () => {
	const plainBoxVertexCount = new THREE.BoxGeometry(1, 1, 1).getAttribute('position').count;

	it('chest is bottom-anchored, reaches full height, and is composed of more than a box', () => {
		const g = buildArchetypeGeometry('chest', 100, 60, 80);
		const b = bbox(g);
		expect(b.min.y).toBeCloseTo(0, 3);
		expect(b.max.y).toBeCloseTo(80, 3);
		expect(g.getAttribute('position').count).toBeGreaterThan(plainBoxVertexCount);
	});

	it('workstation is bottom-anchored and reaches full height', () => {
		const g = buildArchetypeGeometry('workstation', 150, 100, 90);
		const b = bbox(g);
		expect(b.min.y).toBeCloseTo(0, 3);
		expect(b.max.y).toBeCloseTo(90, 3);
		expect(g.getAttribute('position').count).toBeGreaterThan(plainBoxVertexCount);
	});

	it('lampPost is bottom-anchored and extends above the post height', () => {
		const g = buildArchetypeGeometry('lampPost', 40, 40, 300);
		const b = bbox(g);
		expect(b.min.y).toBeCloseTo(0, 2);
		expect(b.max.y).toBeGreaterThan(300);
	});

	it('torch is bottom-anchored and extends above the post height', () => {
		const g = buildArchetypeGeometry('torch', 30, 30, 150);
		const b = bbox(g);
		expect(b.min.y).toBeCloseTo(0, 2);
		expect(b.max.y).toBeGreaterThan(150 * 0.8);
	});

	it('turret is bottom-anchored and composed of more than a box', () => {
		const g = buildArchetypeGeometry('turret', 120, 120, 200);
		const b = bbox(g);
		expect(b.min.y).toBeCloseTo(0, 2);
		expect(g.getAttribute('position').count).toBeGreaterThan(plainBoxVertexCount);
	});

	it('tank is bottom-anchored and reaches full height', () => {
		const g = buildArchetypeGeometry('tank', 100, 100, 250);
		const b = bbox(g);
		expect(b.min.y).toBeCloseTo(0, 2);
		expect(b.max.y).toBeCloseTo(250, 2);
	});

	it('chimneyStack is bottom-anchored and extends above the body height', () => {
		const g = buildArchetypeGeometry('chimneyStack', 100, 100, 200);
		const b = bbox(g);
		expect(b.min.y).toBeCloseTo(0, 3);
		expect(b.max.y).toBeGreaterThan(200 * 0.7);
	});

	it('planter renders as a correctly-sized bottom-anchored box', () => {
		const g = buildArchetypeGeometry('planter', 80, 80, 60);
		const b = bbox(g);
		expect(b.min.y).toBeCloseTo(0, 5);
		expect(b.max.y).toBeCloseTo(60, 5);
		expect(b.max.x - b.min.x).toBeCloseTo(80, 5);
		expect(b.max.z - b.min.z).toBeCloseTo(80, 5);
	});

	it('ladder is bottom-anchored, reaches full height, and is composed of more than a box', () => {
		const g = buildArchetypeGeometry('ladder', 60, 20, 200);
		const b = bbox(g);
		expect(b.min.y).toBeCloseTo(0, 3);
		expect(b.max.y).toBeCloseTo(200, 3);
		expect(g.getAttribute('position').count).toBeGreaterThan(plainBoxVertexCount);
	});
});
