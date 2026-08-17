import { describe, expect, it } from 'vitest';
import { WATCHTOWER_CLASS } from './fastTravel';
import {
	areaFastTravelGuids,
	orderedRelicTypes,
	pointsInArea,
	relicTypeStats,
	unlockedInArea
} from './mapCounts';

// Chosen from the DT_WorldMapUIData bounds in utils.ts: the origin sits inside
// MainMap only, and (500000, -600000) inside Tree only.
const MAIN = { x: 0, y: 0 };
const TREE = { x: 500000, y: -600000 };

const STATUE = 'BP_LevelObject_TowerFastTravelPoint_C';

describe('pointsInArea', () => {
	it('keeps only points whose world position resolves to the area', () => {
		const points = { a: { ...MAIN, id: 'a' }, b: { ...TREE, id: 'b' } };
		expect(pointsInArea(points, 'MainMap').map((p) => p.id)).toEqual(['a']);
		expect(pointsInArea(points, 'Tree').map((p) => p.id)).toEqual(['b']);
	});
});

describe('areaFastTravelGuids', () => {
	const points = {
		'aaa-1': { ...MAIN, class: STATUE },
		'bbb-2': { ...MAIN, class: WATCHTOWER_CLASS },
		'ccc-3': { ...TREE, class: WATCHTOWER_CLASS }
	};

	it('splits statues from watchtowers within one area', () => {
		expect(areaFastTravelGuids(points, 'MainMap', false)).toEqual(new Set(['AAA-1']));
		expect(areaFastTravelGuids(points, 'MainMap', true)).toEqual(new Set(['BBB-2']));
	});

	it('scopes to the requested area', () => {
		expect(areaFastTravelGuids(points, 'Tree', true)).toEqual(new Set(['CCC-3']));
		expect(areaFastTravelGuids(points, 'Tree', false)).toEqual(new Set());
	});
});

describe('unlockedInArea', () => {
	const guids = new Set(['AAA-1', 'BBB-2']);

	it('is undefined when no player is selected', () => {
		expect(unlockedInArea(guids, undefined)).toBeUndefined();
	});

	it('counts case-insensitively and ignores out-of-area unlocks', () => {
		expect(unlockedInArea(guids, ['aaa-1', 'zzz-9'])).toBe(1);
	});

	it('is zero for a player who unlocked nothing here', () => {
		expect(unlockedInArea(guids, [])).toBe(0);
	});
});

describe('relicTypeStats', () => {
	const points = {
		'r-1': { ...MAIN, relic_type: 'capture_power' },
		'r-2': { ...MAIN, relic_type: 'capture_power' },
		'r-3': { ...MAIN, relic_type: 'sealed_realm' },
		'r-4': { ...TREE, relic_type: 'capture_power' }
	};

	it('totals per type within the area, with no player', () => {
		expect(relicTypeStats(points, 'MainMap')).toEqual({
			capture_power: { total: 2, collected: 0 },
			sealed_realm: { total: 1, collected: 0 }
		});
	});

	it('counts collected relics for the selected player', () => {
		const player = { collected_relics: { capture_power: ['R-1'], sealed_realm: [] } };
		expect(relicTypeStats(points, 'MainMap', player)).toEqual({
			capture_power: { total: 2, collected: 1 },
			sealed_realm: { total: 1, collected: 0 }
		});
	});

	it('does not credit a collected relic that lives in another area', () => {
		const player = { collected_relics: { capture_power: ['R-4'] } };
		expect(relicTypeStats(points, 'MainMap', player).capture_power).toEqual({
			total: 2,
			collected: 0
		});
	});
});

describe('orderedRelicTypes', () => {
	it('follows game order, then appends unknown types', () => {
		const stats = { sealed_realm: {}, mystery: {}, capture_power: {} };
		const order = ['capture_power', 'sealed_realm', 'absent_type'];
		expect(orderedRelicTypes(stats, order)).toEqual(['capture_power', 'sealed_realm', 'mystery']);
	});
});
