import { describe, expect, it } from 'vitest';
import { structureInfo } from './structureInfo';

const structure = (over = {}) => ({
	instance_id: 'i1',
	map_object_id: 'BlastFurnace',
	x: 0,
	y: 0,
	z: 0,
	yaw: 0,
	scale_x: 1,
	scale_y: 1,
	scale_z: 1,
	hp_current: 850,
	hp_max: 1000,
	build_player_uid: 'ABC-123',
	...over
});
const footprints = {
	BlastFurnace: { sx: 168, sy: 100, sz: 82, ox: 0, oy: 0, oz: 0, typeA: 'Product' }
};
const buildings = {
	BlastFurnace: {
		localized_name: 'Blast Furnace',
		description: 'Smelts ore.',
		rank: 2,
		icon: 't_icon'
	}
} as never;
const summaries = { 'ABC-123': { uid: 'ABC-123', nickname: 'Omar' } } as never;

describe('structureInfo', () => {
	it('prefers the localized building name', () => {
		expect(structureInfo(structure(), footprints, buildings, summaries).name).toBe('Blast Furnace');
	});

	it('falls back to the raw map_object_id when the building is unknown', () => {
		const info = structureInfo(
			structure({ map_object_id: 'DamagableRock0001' }),
			footprints,
			buildings,
			summaries
		);
		expect(info.name).toBe('DamagableRock0001');
	});

	it('falls back to the Other type when the footprint is unknown', () => {
		const info = structureInfo(
			structure({ map_object_id: 'Nope' }),
			footprints,
			buildings,
			summaries
		);
		expect(info.typeA).toBe('Other');
	});

	it('converts footprint centimetres to metres, applying scale', () => {
		const info = structureInfo(structure({ scale_z: 2 }), footprints, buildings, summaries);
		expect(info.sizeM.x).toBeCloseTo(1.68, 6);
		expect(info.sizeM.z).toBeCloseTo(1.64, 6);
	});

	it('resolves the builder nickname case-insensitively', () => {
		const info = structureInfo(
			structure({ build_player_uid: 'abc-123' }),
			footprints,
			buildings,
			summaries
		);
		expect(info.builder).toBe('Omar');
	});

	it('leaves the builder undefined when the uid is unknown', () => {
		const info = structureInfo(
			structure({ build_player_uid: 'ZZZ' }),
			footprints,
			buildings,
			summaries
		);
		expect(info.builder).toBeUndefined();
	});
});
