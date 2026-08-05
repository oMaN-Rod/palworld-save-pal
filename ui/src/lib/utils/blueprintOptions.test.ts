import { describe, expect, it } from 'vitest';
import { captureOptionsForPreset, CAPTURE_OPTION_FIELDS } from './blueprintOptions';
import type { CaptureOptions } from '$types';

const KEYS: (keyof CaptureOptions)[] = [
	'production_config', 'structure_condition', 'container_contents', 'worker_pals',
	'housed_pals', 'production_progress', 'access_config', 'base_identity'
];

const on = (o: CaptureOptions) => KEYS.filter((k) => o[k]);

describe('captureOptionsForPreset', () => {
	it('blueprint captures only production_config', () => {
		expect(on(captureOptionsForPreset('blueprint'))).toEqual(['production_config']);
	});

	it('configured adds structure_condition, access_config, base_identity', () => {
		expect(on(captureOptionsForPreset('configured')).sort()).toEqual(
			['access_config', 'base_identity', 'production_config', 'structure_condition'].sort()
		);
	});

	it('full captures all eight', () => {
		expect(on(captureOptionsForPreset('full'))).toEqual(KEYS);
	});

	it('every preset returns an object with exactly the eight known keys', () => {
		for (const preset of ['blueprint', 'configured', 'full'] as const) {
			expect(Object.keys(captureOptionsForPreset(preset)).sort()).toEqual([...KEYS].sort());
		}
	});
});

describe('CAPTURE_OPTION_FIELDS', () => {
	it('describes all eight options in a stable order with non-empty copy', () => {
		expect(CAPTURE_OPTION_FIELDS.map((f) => f.key)).toEqual(KEYS);
		for (const field of CAPTURE_OPTION_FIELDS) {
			expect(field.label.length).toBeGreaterThan(0);
			expect(field.description.length).toBeGreaterThan(0);
		}
	});
});
