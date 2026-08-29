import { describe, expect, it } from 'vitest';
import { WATCHTOWER_CLASS, isWatchtower } from './fastTravel';

describe('isWatchtower', () => {
	it('is true for the watchtower class', () => {
		expect(isWatchtower({ class: WATCHTOWER_CLASS })).toBe(true);
	});

	it('is false for the regular tower fast travel class', () => {
		expect(isWatchtower({ class: 'BP_LevelObject_TowerFastTravelPoint_C' })).toBe(false);
	});

	it('is false when class is missing', () => {
		expect(isWatchtower({})).toBe(false);
	});
});
