import { describe, it, expect } from 'vitest';
import {
	MAP_OBJECT_SCALE_MIN,
	MAP_OBJECT_SCALE_MAX,
	MAP_OBJECT_SCALE_DEFAULT,
	sliderToScale,
	scaleToSlider
} from './mapObjectSize';

describe('map object scale bounds', () => {
	it('starts at true-to-scale', () => {
		expect(MAP_OBJECT_SCALE_MIN).toBe(1);
		expect(sliderToScale(0)).toBeCloseTo(1, 10);
	});

	it('ends at the maximum', () => {
		expect(MAP_OBJECT_SCALE_MAX).toBe(60);
		expect(sliderToScale(1)).toBeCloseTo(60, 10);
	});

	// A fast travel statue is 375 cm, about the size of a 3 m Pal, and Pals
	// default to 30x. At 1x the statues would be dwarfed by the Pals beside them.
	it('defaults to a landmark size below the Pal default', () => {
		expect(MAP_OBJECT_SCALE_DEFAULT).toBe(20);
		expect(MAP_OBJECT_SCALE_DEFAULT).toBeLessThan(30);
	});

	it('round-trips the default through the slider position', () => {
		expect(sliderToScale(scaleToSlider(MAP_OBJECT_SCALE_DEFAULT))).toBeCloseTo(
			MAP_OBJECT_SCALE_DEFAULT,
			8
		);
	});

	it('clamps out-of-range positions', () => {
		expect(sliderToScale(-1)).toBe(MAP_OBJECT_SCALE_MIN);
		expect(sliderToScale(2)).toBe(MAP_OBJECT_SCALE_MAX);
	});
});
