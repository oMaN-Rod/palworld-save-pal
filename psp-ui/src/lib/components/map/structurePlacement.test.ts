import { describe, it, expect } from 'vitest';
import { structurePlacement } from './structurePlacement';
import { verticalScaleFactor } from './mercator';
import { cmPerPx } from './utils';
import type { BaseStructure, Footprint } from '$types';

const fp: Footprint = { sx: 400, sy: 20, sz: 325, ox: 0, oy: 0, oz: 0, typeA: 'Foundation', archetype: 'wall', material: 'Wood' };
const base = (over: Partial<BaseStructure>): BaseStructure => ({
	instance_id: 'i', map_object_id: 'Wooden_Wall', x: 0, y: 0, z: 0, yaw: 0,
	scale_x: 1, scale_y: 1, scale_z: 1, hp_current: 1, hp_max: 1, build_player_uid: 'u', ...over
});

describe('structurePlacement', () => {
	it('scales footprint by per-axis scale', () => {
		const vs = verticalScaleFactor(0, cmPerPx('MainMap'));
		const p = structurePlacement(base({ scale_x: 2, scale_z: 3 }), fp, 'MainMap', vs);
		expect(p.footprintCm.sx).toBeCloseTo(800, 5);
		expect(p.footprintCm.sz).toBeCloseTo(975, 5);
	});

	it('carries world Z (plus oz) as altitude in cm', () => {
		const vs = verticalScaleFactor(0, cmPerPx('MainMap'));
		const p = structurePlacement(base({ z: 5000 }), { ...fp, oz: 100 }, 'MainMap', vs);
		expect(p.altitudeCm).toBeCloseTo(5100, 5);
	});

	it('produces a lng/lat inside world bounds for the origin', () => {
		const vs = verticalScaleFactor(0, cmPerPx('MainMap'));
		const p = structurePlacement(base({}), fp, 'MainMap', vs);
		expect(p.lng).toBeGreaterThan(-180);
		expect(p.lng).toBeLessThan(180);
		expect(p.lat).toBeGreaterThan(-85.1);
		expect(p.lat).toBeLessThan(85.1);
	});
});
