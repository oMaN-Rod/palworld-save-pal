import { describe, expect, it } from 'vitest';
import { isNewerVersion } from './appVersion';

describe('isNewerVersion', () => {
	it('compares release versions field by field', () => {
		expect(isNewerVersion('1.5.0', '1.4.2')).toBe(true);
		expect(isNewerVersion('1.4.2', '1.5.0')).toBe(false);
		expect(isNewerVersion('1.4.10', '1.4.9')).toBe(true);
		expect(isNewerVersion('1.5.0', '1.5.0')).toBe(false);
	});

	it('ranks a release above its own pre-releases', () => {
		expect(isNewerVersion('1.5.0', '1.5.0-beta.1')).toBe(true);
		expect(isNewerVersion('1.5.0-beta.1', '1.5.0')).toBe(false);
	});

	it('orders pre-releases of the same version', () => {
		expect(isNewerVersion('1.5.0-beta.2', '1.5.0-beta.1')).toBe(true);
		expect(isNewerVersion('1.5.0-beta.1', '1.5.0-beta.2')).toBe(false);
		expect(isNewerVersion('1.5.0-beta.1', '1.5.0-alpha.9')).toBe(true);
		expect(isNewerVersion('1.5.0-beta.1', '1.5.0-beta.1')).toBe(false);
	});

	it('ranks numeric identifiers below alphanumeric ones', () => {
		expect(isNewerVersion('1.5.0-beta', '1.5.0-1')).toBe(true);
		expect(isNewerVersion('1.5.0-1', '1.5.0-beta')).toBe(false);
	});

	it('ranks a longer identifier list above its own prefix', () => {
		expect(isNewerVersion('1.5.0-beta.1.1', '1.5.0-beta.1')).toBe(true);
		expect(isNewerVersion('1.5.0-beta.1', '1.5.0-beta.1.1')).toBe(false);
	});

	it('compares across versions regardless of pre-release tags', () => {
		expect(isNewerVersion('1.5.0-beta.1', '1.4.2')).toBe(true);
		expect(isNewerVersion('1.4.2', '1.5.0-beta.1')).toBe(false);
	});

	it('ignores build metadata', () => {
		expect(isNewerVersion('1.5.0+2026', '1.5.0')).toBe(false);
		expect(isNewerVersion('1.5.0', '1.5.0+2026')).toBe(false);
		expect(isNewerVersion('1.5.1+2026', '1.5.0')).toBe(true);
	});

	it('treats missing trailing fields as zero', () => {
		expect(isNewerVersion('1.5', '1.5.0')).toBe(false);
		expect(isNewerVersion('1.5.1', '1.5')).toBe(true);
	});
});
