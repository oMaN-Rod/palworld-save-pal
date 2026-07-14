import { describe, expect, it } from 'vitest';
import {
	CAPTURE_POWER,
	collectRelics,
	relicsByType,
	toggleRelic,
	type RelicPlayerView
} from './relics';

const A = 'AAAAAAAA-0000-0000-0000-000000000001';
const B = 'BBBBBBBB-0000-0000-0000-000000000002';
const C = 'CCCCCCCC-0000-0000-0000-000000000003';
const D = 'DDDDDDDD-0000-0000-0000-000000000004';

/** A pre-1.0 save: effigies in the flat list, no by-type relic structure at all. */
const preOnePointZero = (): RelicPlayerView => ({
	collected_effigies: [A, B, C],
	collected_relics: {}
});

describe('relicsByType', () => {
	it('seeds capture_power from collected_effigies on a pre-1.0 save', () => {
		expect(relicsByType(preOnePointZero())[CAPTURE_POWER]).toEqual([A, B, C]);
	});

	it('leaves an existing by-type list alone on a 1.0 save', () => {
		const player: RelicPlayerView = {
			collected_effigies: [A, B],
			collected_relics: { [CAPTURE_POWER]: [A, B], defense_power: [C] }
		};
		expect(relicsByType(player)).toEqual({ [CAPTURE_POWER]: [A, B], defense_power: [C] });
	});

	it('is empty for a fresh player', () => {
		expect(relicsByType({ collected_effigies: [], collected_relics: {} })[CAPTURE_POWER]).toEqual(
			[]
		);
	});
});

describe('toggleRelic on a pre-1.0 save', () => {
	it('keeps every existing effigy when a new one is collected', () => {
		const player = preOnePointZero();
		const delta = toggleRelic(player, { guid: D, relic_type: CAPTURE_POWER });
		expect(delta).toBe(1);
		expect(player.collected_effigies).toEqual(expect.arrayContaining([A, B, C, D]));
		expect(player.collected_effigies).toHaveLength(4);
		expect(player.collected_relics?.[CAPTURE_POWER]).toEqual([A, B, C, D]);
	});

	it('removes only the toggled-off effigy', () => {
		const player = preOnePointZero();
		const delta = toggleRelic(player, { guid: A, relic_type: CAPTURE_POWER });
		expect(delta).toBe(-1);
		expect(player.collected_effigies).toEqual([B, C]);
		expect(player.collected_relics?.[CAPTURE_POWER]).toEqual([B, C]);
	});

	it('does not touch collected_effigies for a non-capture_power relic', () => {
		const player = preOnePointZero();
		const delta = toggleRelic(player, { guid: D, relic_type: 'defense_power' });
		expect(delta).toBe(0);
		expect(player.collected_effigies).toEqual([A, B, C]);
		expect(player.collected_relics?.defense_power).toEqual([D]);
	});
});

describe('collectRelics', () => {
	it('returns only the newly added capture_power count on a pre-1.0 save', () => {
		const player = preOnePointZero();
		const points = [A, B, C, D].map((guid) => ({ guid, relic_type: CAPTURE_POWER }));
		expect(collectRelics(player, points)).toEqual({ added: 1, capturePowerAdded: 1 });
		expect(player.collected_effigies).toEqual([A, B, C, D]);
	});

	it('is a no-op when everything is already collected', () => {
		const player = preOnePointZero();
		const points = [A, B, C].map((guid) => ({ guid, relic_type: CAPTURE_POWER }));
		expect(collectRelics(player, points)).toEqual({ added: 0, capturePowerAdded: 0 });
		expect(player.collected_effigies).toEqual([A, B, C]);
	});

	it('counts case-insensitively against existing GUIDs', () => {
		const player = preOnePointZero();
		const points = [{ guid: A.toLowerCase(), relic_type: CAPTURE_POWER }];
		expect(collectRelics(player, points).added).toBe(0);
	});
});
