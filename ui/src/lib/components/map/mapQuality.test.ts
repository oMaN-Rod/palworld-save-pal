import { describe, expect, it } from 'vitest';
import {
	autoQualityStep,
	createAutoQualityState,
	isMapQualitySetting,
	MAP_QUALITY_DEFAULT,
	MAP_QUALITY_LEVELS,
	qualityParams
} from './mapQuality';

describe('qualityParams', () => {
	it('the default tier reproduces the pre-quality-settings rendering', () => {
		// pixelRatio null = device default; 10px scenery cull was the shipped
		// constant; structures honour the user's own mode.
		expect(MAP_QUALITY_DEFAULT).toBe('high');
		expect(qualityParams('high', 2)).toEqual({
			pixelRatio: null,
			sceneryMinPixels: 10,
			forceStructuresProxy: false,
			meshSweepAgeMs: 60_000
		});
	});

	it('scenery density rises monotonically with the tier', () => {
		const pixels = MAP_QUALITY_LEVELS.map((level) => qualityParams(level, 2).sceneryMinPixels);
		for (let i = 1; i < pixels.length; i++) {
			expect(pixels[i]).toBeLessThan(pixels[i - 1]);
		}
	});

	it('resolution never drops below 1 and only very-high exceeds the device ratio', () => {
		expect(qualityParams('very-low', 3).pixelRatio).toBe(1);
		expect(qualityParams('medium', 3).pixelRatio).toBe(1.5);
		expect(qualityParams('high', 3).pixelRatio).toBeNull();
		// Supersampling caps at 2.75 absolute but never below the device ratio.
		expect(qualityParams('very-high', 2).pixelRatio).toBe(2.5);
		expect(qualityParams('very-high', 1).pixelRatio).toBe(1.25);
		expect(qualityParams('very-high', 3).pixelRatio).toBe(3);
	});

	it('only the two low tiers force structure proxies', () => {
		for (const level of MAP_QUALITY_LEVELS) {
			const force = qualityParams(level, 1).forceStructuresProxy;
			expect(force).toBe(level === 'very-low' || level === 'low');
		}
	});

	it('a non-finite device ratio falls back to 1 rather than NaN', () => {
		expect(qualityParams('medium', Number.NaN).pixelRatio).toBe(1);
	});
});

describe('autoQualityStep', () => {
	it('steps down after sustained low fps and then cools down', () => {
		let state = createAutoQualityState('high');
		let now = 0;
		for (let i = 0; i < 2; i++) {
			const r = autoQualityStep(state, 30, (now += 500));
			expect(r.changed).toBe(false);
			state = r.state;
		}
		const step = autoQualityStep(state, 30, (now += 500));
		expect(step.changed).toBe(true);
		expect(step.level).toBe('medium');
		// Inside the cooldown window nothing changes, even under load.
		const cooled = autoQualityStep(step.state, 5, now + 1_000);
		expect(cooled.changed).toBe(false);
	});

	it('never steps below very-low', () => {
		let state = createAutoQualityState('very-low');
		for (let i = 0; i < 10; i++) {
			const r = autoQualityStep(state, 1, i * 500);
			state = r.state;
			expect(r.level).toBe('very-low');
		}
	});

	it('steps up on sustained headroom but caps at high', () => {
		let state = createAutoQualityState('medium');
		let now = 0;
		for (let i = 0; i < 6; i++) {
			const r = autoQualityStep(state, 120, (now += 500));
			state = r.state;
			if (r.changed) break;
		}
		expect(state.level).toBe('high');
		// Already at the auto ceiling: more headroom changes nothing.
		const ceiling = autoQualityStep(state, 120, now + 20_000);
		expect(ceiling.level).toBe('high');
		expect(autoQualityStep(ceiling.state, 120, ceiling.state.cooldownUntil + 1).level).toBe('high');
	});

	it('idle windows count as headroom, not as load', () => {
		let state = createAutoQualityState('medium');
		let now = 0;
		for (let i = 0; i < 6; i++) {
			const r = autoQualityStep(state, null, (now += 500));
			state = r.state;
		}
		expect(state.level).toBe('high');
	});

	it('mid-range fps resets both counters so a steady 50fps never oscillates', () => {
		let state = createAutoQualityState('medium');
		for (let i = 0; i < 20; i++) {
			const r = autoQualityStep(state, 50, i * 500);
			state = r.state;
			expect(r.changed).toBe(false);
			expect(state.pressure).toBe(0);
			expect(state.relax).toBe(0);
		}
		expect(state.level).toBe('medium');
	});
});

describe('isMapQualitySetting', () => {
	it('accepts every level and auto, rejects anything else', () => {
		for (const level of [...MAP_QUALITY_LEVELS, 'auto']) {
			expect(isMapQualitySetting(level)).toBe(true);
		}
		expect(isMapQualitySetting('ultra')).toBe(false);
		expect(isMapQualitySetting(3)).toBe(false);
	});
});
