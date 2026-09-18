import { describe, expect, it } from 'vitest';
import { nexusDateText, nexusErrorText } from './nexusText';

describe('nexusErrorText', () => {
	it('has its own sentence for the codes a user can act on', () => {
		expect(nexusErrorText({ code: 'key_required', message: 'add a key' })).toContain('API key');
		expect(nexusErrorText({ code: 'invalid_key', message: 'nope' })).toContain('did not accept');
		expect(nexusErrorText({ code: 'keyring_unavailable', message: 'boom' })).toContain('keyring');
		expect(nexusErrorText({ code: 'desktop_only', message: 'no' })).toContain('desktop app');
		expect(nexusErrorText({ code: 'network', message: 'no route' })).toContain('could not be reached');
	});

	it('names the reset when a rate limit carries one', () => {
		expect(nexusErrorText({ code: 'rate_limited', message: 'slow down', reset: '2026-09-18T00:00:00+00:00' })).toContain('2026');
		expect(nexusErrorText({ code: 'rate_limited', message: 'slow down' })).toContain('later');
	});

	it('falls back to the server sentence for anything else', () => {
		expect(nexusErrorText({ code: 'weird_code', message: 'something specific' })).toBe('something specific');
	});

	it('has its own sentence for the handler-registration codes', () => {
		expect(nexusErrorText({ code: 'no_executable', message: 'nope' })).toContain('program file');
		expect(nexusErrorText({ code: 'register_failed', message: 'nope' })).toContain('Registering');
		expect(nexusErrorText({ code: 'unsupported_platform', message: 'nope' })).toContain('platform');
	});
});

describe('nexusDateText', () => {
	const now = Date.parse('2026-09-18T12:00:00Z');

	it('counts back in the largest unit that still fits', () => {
		expect(nexusDateText('2026-09-15T12:00:00Z', now)).toBe('3 days ago');
		expect(nexusDateText('2026-09-18T09:00:00Z', now)).toBe('3 hours ago');
		expect(nexusDateText('2026-09-18T11:58:00Z', now)).toBe('2 minutes ago');
		expect(nexusDateText('2026-09-18T11:59:59Z', now)).toBe('1 second ago');
	});

	it('switches to a plain date once a month has passed', () => {
		expect(nexusDateText('2026-01-04T12:00:00Z', now)).toContain('2026');
		expect(nexusDateText('2026-01-04T12:00:00Z', now)).not.toContain('ago');
	});

	it('has nothing to say about a missing or unreadable date', () => {
		expect(nexusDateText(null, now)).toBeNull();
		expect(nexusDateText('not a date', now)).toBeNull();
	});
});
