import { describe, expect, it } from 'vitest';
import { osFamilyFrom } from './platform';

describe('osFamilyFrom', () => {
	it('detects Windows', () => {
		expect(osFamilyFrom('Mozilla/5.0 (Windows NT 10.0; Win64; x64)')).toBe('windows');
	});

	it('detects macOS', () => {
		expect(osFamilyFrom('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)')).toBe('macos');
	});

	it('detects Linux', () => {
		expect(osFamilyFrom('Mozilla/5.0 (X11; Linux x86_64)')).toBe('linux');
	});

	it('returns other for anything unrecognised', () => {
		expect(osFamilyFrom('Mozilla/5.0 (PlayStation 5)')).toBe('other');
	});

	it('prefers Windows over the Linux substring in a WSL-style agent', () => {
		expect(osFamilyFrom('Mozilla/5.0 (Windows NT 10.0) Linux')).toBe('windows');
	});
});
