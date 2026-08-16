import { describe, expect, it } from 'vitest';
import { groupPassiveFamilies, stripRankSuffix } from './passiveFamilies';
import type { PassiveSkill } from '$types';

const makeSkill = (name: string, rank: number): PassiveSkill =>
	({
		id: name,
		localized_name: name,
		description: '',
		details: { rank, effects: [] }
	}) as PassiveSkill;

const entry = (key: string, name: string, rank: number): [string, PassiveSkill] => [
	key,
	makeSkill(name, rank)
];

describe('stripRankSuffix', () => {
	it('strips "Lv. N" suffixes', () => {
		expect(stripRankSuffix('Attack Up Lv. 2')).toBe('Attack Up');
		expect(stripRankSuffix('Attack Up Lv.3')).toBe('Attack Up');
	});

	it('strips "+N" suffixes', () => {
		expect(stripRankSuffix('Aerial Dash +3')).toBe('Aerial Dash');
	});

	it('leaves rank-less names untouched', () => {
		expect(stripRankSuffix('Legend')).toBe('Legend');
		expect(stripRankSuffix('Heavyweight')).toBe('Heavyweight');
	});
});

describe('groupPassiveFamilies', () => {
	it('collapses rank variants sharing a base name into one family', () => {
		const families = groupPassiveFamilies([
			entry('Attack_ACC_up2', 'Attack Up Lv. 2', 2),
			entry('Attack_ACC_up3', 'Attack Up Lv. 3', 3),
			entry('Attack_ACC_up4', 'Attack Up Lv. 4', 4)
		]);
		expect(families).toHaveLength(1);
		expect(families[0].displayName).toBe('Attack Up');
		expect(families[0].ranks).toEqual([2, 3, 4]);
		expect(families[0].primaryRank).toBe(4);
		expect(families[0].members).toHaveLength(3);
	});

	it('keeps same-name same-rank stack variants as a single-rank family', () => {
		const families = groupPassiveFamilies([
			entry('AirDash_1', 'Aerial Dash +1', 1),
			entry('AirDash_2', 'Aerial Dash +2', 1),
			entry('AirDash_3', 'Aerial Dash +3', 1),
			entry('AirDash_4', 'Aerial Dash +4', 1)
		]);
		expect(families).toHaveLength(1);
		expect(families[0].ranks).toEqual([1]);
		expect(families[0].members).toHaveLength(4);
	});

	it('keeps unrelated names separate even if keys share a prefix', () => {
		// Deffence_up1/2/2_2/3 have totally different display names in-game.
		const families = groupPassiveFamilies([
			entry('Deffence_up1', 'Hard Skin', 1),
			entry('Deffence_up2', 'Burly Body', 3),
			entry('Deffence_up2_2', 'Heavyweight', 2),
			entry('Deffence_up3', 'Diamond Body', 4)
		]);
		expect(families).toHaveLength(4);
	});

	it('singleton stays its own family', () => {
		const families = groupPassiveFamilies([entry('Legend', 'Legend', 4)]);
		expect(families).toHaveLength(1);
		expect(families[0].members).toHaveLength(1);
		expect(families[0].ranks).toEqual([4]);
	});

	it('orders families by primary rank desc, then name asc', () => {
		const families = groupPassiveFamilies([
			entry('a', 'Alpha Boost Lv. 2', 2),
			entry('b', 'Beta Boost Lv. 4', 4),
			entry('c', 'Gamma Boost Lv. 4', 4)
		]);
		expect(families.map((f) => f.displayName)).toEqual([
			'Beta Boost',
			'Gamma Boost',
			'Alpha Boost'
		]);
	});

	it('sorts members within a family by rank then key', () => {
		const families = groupPassiveFamilies([
			entry('X_4', 'Surge Lv. 4', 4),
			entry('X_2', 'Surge Lv. 2', 2),
			entry('X_3', 'Surge Lv. 3', 3)
		]);
		expect(families[0].members.map((m) => m.key)).toEqual(['X_2', 'X_3', 'X_4']);
	});
});
