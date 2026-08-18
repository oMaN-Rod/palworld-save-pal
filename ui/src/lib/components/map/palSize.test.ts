import { describe, expect, it } from 'vitest';
import {
	PAL_SCALE_DEFAULT,
	PAL_SCALE_MAX,
	PAL_SCALE_MIN,
	scaleToSlider,
	sliderToScale
} from './palSize';

describe('slider bounds', () => {
	it('starts at true-to-scale', () => {
		expect(PAL_SCALE_MIN).toBe(1);
		expect(sliderToScale(0)).toBeCloseTo(1, 10);
	});

	it('ends at the maximum', () => {
		expect(sliderToScale(1)).toBeCloseTo(PAL_SCALE_MAX, 10);
	});

	it('keeps today-s scale as the default', () => {
		expect(PAL_SCALE_DEFAULT).toBe(30);
	});
});

describe('log mapping', () => {
	it('round-trips a scale through the slider position', () => {
		for (const scale of [1, 3, 10, 30, 60]) {
			expect(sliderToScale(scaleToSlider(scale))).toBeCloseTo(scale, 8);
		}
	});

	it('puts the geometric midpoint at the halfway position', () => {
		expect(sliderToScale(0.5)).toBeCloseTo(Math.sqrt(PAL_SCALE_MIN * PAL_SCALE_MAX), 8);
	});

	it('gives the default most of the travel, unlike a linear slider', () => {
		// A linear 1..60 slider would put 30x at 0.49; the log slider pushes it
		// past 0.8, so the usable range around it gets real resolution.
		expect(scaleToSlider(PAL_SCALE_DEFAULT)).toBeGreaterThan(0.8);
	});

	it('clamps out-of-range positions instead of extrapolating', () => {
		expect(sliderToScale(-1)).toBe(PAL_SCALE_MIN);
		expect(sliderToScale(2)).toBe(PAL_SCALE_MAX);
	});
});
