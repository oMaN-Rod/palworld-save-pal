import { getLocale, overwriteGetLocale } from '$i18n/runtime';
import { afterEach, describe, expect, it } from 'vitest';
import { c, p } from './commonTranslations';

const original = getLocale;

afterEach(() => {
	overwriteGetLocale(original);
});

describe('commonTranslations', () => {
	it('re-reads a term after the locale changes', () => {
		overwriteGetLocale(() => 'en');
		expect(c.storage).toBe('Storage');
		overwriteGetLocale(() => 'de');
		expect(c.storage).toBe('Lagerung');
	});

	it('re-reads interpolation operands after the locale changes', () => {
		overwriteGetLocale(() => 'en');
		expect(c.container).toBe('Storage Container');
		overwriteGetLocale(() => 'de');
		expect(c.container).toBe('Lagerbehälter');
	});

	it('follows the locale through the message-input helper', () => {
		overwriteGetLocale(() => 'en');
		expect(p.human.human).toBe('Human');
		overwriteGetLocale(() => 'de');
		expect(p.human.human).toBe('Menschlich');
	});
});
