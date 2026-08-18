import type { RelicRankData } from '$lib/data/relic.svelte';
import { describe, expect, it } from 'vitest';
import {
	RELIC_ORDER,
	clampCount,
	deriveStatusPatches,
	isIllegalCount,
	rankForCount,
	relicIconPath,
	relicIndexLabel,
	statKeyFor
} from './effigies';

describe('RELIC_ORDER', () => {
	it('has the 13 types in EPalRelicType order', () => {
		expect(RELIC_ORDER).toHaveLength(13);
		expect(RELIC_ORDER[0]).toBe('capture_power');
		expect(RELIC_ORDER[12]).toBe('move_speed');
	});

	it('matches the icon numbering PalSavTools uses', () => {
		expect(relicIconPath('capture_power')).toMatch(/t_itemicon_relic\.webp$/);
		expect(relicIconPath('hunger_reduction')).toMatch(/t_itemicon_relic_01\.webp$/);
		expect(relicIconPath('move_speed')).toMatch(/t_itemicon_relic_12\.webp$/);
		expect(relicIndexLabel('capture_power')).toBe('00');
		expect(relicIndexLabel('move_speed')).toBe('12');
		expect(relicIndexLabel('something_new')).toBe('--');
		expect(relicIconPath('something_new')).toMatch(/t_itemicon_relic\.webp$/);
	});
});

describe('statKeyFor', () => {
	it('maps capture_power to capture_rate and everything else 1:1', () => {
		expect(statKeyFor('capture_power')).toBe('capture_rate');
		expect(statKeyFor('swim_speed')).toBe('swim_speed');
	});
});

describe('rankForCount', () => {
	// capture_power's per_rank is [1,2,3,4,5,6,7,9,...]: thresholds 1,3,6,10,15...
	const capturePerRank = [1, 2, 3, 4, 5, 6, 7, 9, 9, 9, 9, 9, 9, 9, 9];

	it('walks cumulative thresholds', () => {
		expect(rankForCount(capturePerRank, 0)).toBe(0);
		expect(rankForCount(capturePerRank, 1)).toBe(1);
		expect(rankForCount(capturePerRank, 2)).toBe(1);
		expect(rankForCount(capturePerRank, 3)).toBe(2);
		expect(rankForCount(capturePerRank, 6)).toBe(3);
		expect(rankForCount(capturePerRank, 10)).toBe(4);
		// Real-save ground truth: 58 collected is rank 10 in-game.
		expect(rankForCount(capturePerRank, 58)).toBe(10);
	});

	it('saturates at the number of steps', () => {
		expect(rankForCount(capturePerRank, 10_000)).toBe(15);
	});
});

describe('clampCount', () => {
	it('clamps to [0, cap]', () => {
		expect(clampCount(-5, 20)).toBe(0);
		expect(clampCount(7, 20)).toBe(7);
		expect(clampCount(999, 20)).toBe(20);
	});
});

describe('isIllegalCount', () => {
	it('flags over-cap counts only when the cap is known', () => {
		expect(isIllegalCount(21, 20)).toBe(true);
		expect(isIllegalCount(20, 20)).toBe(false);
		expect(isIllegalCount(21, undefined)).toBe(false);
	});
});

describe('deriveStatusPatches', () => {
	const relics = {
		capture_power: {
			per_rank: [1, 2, 3],
			cumulative_max: 6,
			max_rank: 3,
			effect_rate: [],
			localized_name: '',
			description: ''
		},
		swim_speed: {
			per_rank: [1, 1],
			cumulative_max: 2,
			max_rank: 2,
			effect_rate: [],
			localized_name: '',
			description: ''
		}
	} as unknown as Record<string, RelicRankData>;

	it('derives the rank of every staged change from its staged count', () => {
		const patches = deriveStatusPatches({ capture_power: 3, swim_speed: 2 }, { capture_power: 1 }, relics);
		expect(patches['capture_rate']).toBe(2);
		expect(patches['swim_speed']).toBe(2);
	});

	it('treats a staged-to-zero change as rank 0', () => {
		const patches = deriveStatusPatches({ swim_speed: 2 }, { capture_power: 2 }, relics);
		expect(patches['capture_rate']).toBe(0);
		expect(patches['swim_speed']).toBe(2);
	});

	it('leaves untouched types out of the patch so bought ranks survive', () => {
		// swim_speed was never touched: its stored rank must not be rewritten
		// from a held count of 0.
		const patches = deriveStatusPatches({ capture_power: 3 }, { capture_power: 1, swim_speed: 0 }, relics);
		expect(patches['capture_rate']).toBe(2);
		expect(patches['swim_speed']).toBeUndefined();
	});

	it('patches nothing when staged equals loaded', () => {
		const patches = deriveStatusPatches({ capture_power: 2 }, { capture_power: 2 }, relics);
		expect(Object.keys(patches)).toHaveLength(0);
	});
});
