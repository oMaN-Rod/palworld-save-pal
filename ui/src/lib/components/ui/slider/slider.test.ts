import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import Slider from './Slider.svelte';
import { coarseStep, fractionOf, quantize, valueFromFraction } from './slider';

describe('quantize', () => {
	it('clamps to the track', () => {
		expect(quantize(-5, 0, 60, 1)).toBe(0);
		expect(quantize(99, 0, 60, 1)).toBe(60);
		expect(quantize(42, 0, 60, 1)).toBe(42);
	});

	it('snaps to the nearest step from min', () => {
		expect(quantize(7, 0, 100, 5)).toBe(5);
		expect(quantize(8, 0, 100, 5)).toBe(10);
		expect(quantize(7, 1, 100, 5)).toBe(6);
	});

	it('keeps max reachable when it is off the step grid', () => {
		expect(quantize(10, 0, 10, 3)).toBe(10);
		expect(quantize(8.6, 0, 10, 3)).toBe(10);
		expect(quantize(8, 0, 10, 3)).toBe(9);
	});

	it('does not leak float noise', () => {
		expect(quantize(0.3, 0, 1, 0.1)).toBe(0.3);
		expect(quantize(0.7000001, 0, 1, 0.1)).toBe(0.7);
	});

	it('survives a degenerate track or step', () => {
		expect(quantize(5, 3, 3, 1)).toBe(3);
		expect(quantize(5, 0, 60, 0)).toBe(5);
		expect(quantize(Number.NaN, 0, 60, 1)).toBe(0);
	});
});

describe('fractionOf', () => {
	it('maps a value onto 0..1', () => {
		expect(fractionOf(0, 0, 60)).toBe(0);
		expect(fractionOf(30, 0, 60)).toBe(0.5);
		expect(fractionOf(60, 0, 60)).toBe(1);
		expect(fractionOf(6, 4, 8)).toBe(0.5);
	});

	it('clamps outside values and treats an empty track as empty', () => {
		expect(fractionOf(-1, 0, 60)).toBe(0);
		expect(fractionOf(61, 0, 60)).toBe(1);
		expect(fractionOf(5, 3, 3)).toBe(0);
	});
});

describe('valueFromFraction', () => {
	it('maps a pointer position back to a stepped value', () => {
		expect(valueFromFraction(0, 0, 60, 1)).toBe(0);
		expect(valueFromFraction(1, 0, 60, 1)).toBe(60);
		expect(valueFromFraction(0.5, 0, 60, 1)).toBe(30);
		expect(valueFromFraction(0.51, 0, 60, 5)).toBe(30);
	});

	it('clamps a fraction dragged past either end', () => {
		expect(valueFromFraction(-0.4, 0, 60, 1)).toBe(0);
		expect(valueFromFraction(1.4, 0, 60, 1)).toBe(60);
	});
});

describe('coarseStep', () => {
	it('is a twentieth of the track, on the step grid', () => {
		expect(coarseStep(0, 60, 1)).toBe(3);
		expect(coarseStep(0, 1000, 1)).toBe(50);
		expect(coarseStep(0, 100, 5)).toBe(5);
	});

	it('never falls below one step', () => {
		expect(coarseStep(0, 10, 1)).toBe(1);
		expect(coarseStep(0, 3, 1)).toBe(1);
		expect(coarseStep(0, 1, 0.1)).toBe(0.1);
	});
});

describe('<Slider>', () => {
	const html = (props: Record<string, unknown> = {}) =>
		render(Slider, { props: { value: 30, max: 60, ...props } }).body;

	it('exposes the track as an ARIA slider', () => {
		const body = html({ label: 'Capture Power' });
		expect(body).toMatch(/role="slider"/);
		expect(body).toMatch(/aria-valuemin="0"/);
		expect(body).toMatch(/aria-valuemax="60"/);
		expect(body).toMatch(/aria-valuenow="30"/);
		expect(body).toMatch(/aria-label="Capture Power"/);
		expect(body).toMatch(/width: 50%/);
	});

	it('recolors the fill at max only when asked to', () => {
		expect(html({ value: 60, completeColor: 'success' })).toMatch(/bg-success-500/);
		expect(html({ value: 59, completeColor: 'success' })).toMatch(/bg-secondary-500/);
		expect(html({ value: 60 })).not.toMatch(/bg-success-500/);
	});

	it('quantizes a value off the step grid before showing it', () => {
		expect(html({ value: 7, max: 100, step: 5 })).toMatch(/aria-valuenow="5"/);
	});

	it('disables the whole row, steppers included', () => {
		const body = html({ disabled: true });
		expect(body).toMatch(/aria-disabled="true"/);
		expect(body).toMatch(/tabindex="-1"/);
		expect(body.match(/<button[^>]*\sdisabled>/g)).toHaveLength(2);
	});

	it('disables only the stepper at the end the value sits on', () => {
		expect(html({ value: 0 }).match(/<button[^>]*\sdisabled>/g)).toHaveLength(1);
		expect(html({ value: 60 }).match(/<button[^>]*\sdisabled>/g)).toHaveLength(1);
		expect(html({ value: 30 }).match(/<button[^>]*\sdisabled>/g)).toBeNull();
	});

	it('drops the optional chrome on request', () => {
		const bare = html({ showSteppers: false, showValue: false });
		expect(bare).not.toMatch(/<button/);
		expect(bare).not.toMatch(/>30</);
		expect(html()).not.toMatch(/\/60/);
		expect(html({ showMax: true })).toMatch(/\/60/);
	});

	it('renders xs as a bare rail with a thumb', () => {
		const body = html({ size: 'xs' });
		expect(body).not.toMatch(/<button/);
		expect(body).not.toMatch(/>30</);
		expect(body).toContain('left: calc(0.5 * (100% - 12px))');
	});

	it('lets a compact rail opt back into the chrome', () => {
		const body = html({ size: 'xs', showSteppers: true, showValue: true, thumb: false });
		expect(body).toMatch(/<button/);
		expect(body).toMatch(/>30</);
		expect(body).not.toContain('100% - 12px');
	});

	it('places markers across the track', () => {
		const body = html({ markers: [15, 45] });
		expect(body).toContain('left: 25%');
		expect(body).toContain('left: 75%');
	});

	it('renders and announces a formatted value', () => {
		const body = html({ value: 30, format: (v: number) => `${v}%` });
		expect(body).toMatch(/aria-valuetext="30%"/);
		expect(body).toMatch(/>30%</);
	});
});
