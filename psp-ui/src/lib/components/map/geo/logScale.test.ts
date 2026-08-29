import { describe, it, expect } from 'vitest';
import { sliderToScale, scaleToSlider } from './logScale';

describe('sliderToScale', () => {
	it('maps 0 to the minimum and 1 to the maximum', () => {
		expect(sliderToScale(0, 1, 60)).toBeCloseTo(1, 10);
		expect(sliderToScale(1, 1, 60)).toBeCloseTo(60, 10);
	});

	it('puts the geometric midpoint at the halfway position', () => {
		expect(sliderToScale(0.5, 1, 60)).toBeCloseTo(Math.sqrt(60), 10);
	});

	it('clamps out-of-range positions instead of extrapolating', () => {
		expect(sliderToScale(-1, 1, 60)).toBe(1);
		expect(sliderToScale(2, 1, 60)).toBe(60);
	});

	it('honours a min other than 1', () => {
		expect(sliderToScale(0, 5, 50)).toBeCloseTo(5, 10);
		expect(sliderToScale(1, 5, 50)).toBeCloseTo(50, 10);
	});
});

describe('scaleToSlider', () => {
	it('round-trips through sliderToScale', () => {
		for (const scale of [1, 3, 10, 20, 30, 60]) {
			expect(sliderToScale(scaleToSlider(scale, 1, 60), 1, 60)).toBeCloseTo(scale, 8);
		}
	});

	it('clamps a scale outside the range', () => {
		expect(scaleToSlider(0.1, 1, 60)).toBe(0);
		expect(scaleToSlider(1000, 1, 60)).toBe(1);
	});
});
