import { describe, expect, it } from 'vitest';
import {
	isCompatExemptRoute,
	isFullBleedRoute,
	isPipWindow,
	isPublicShell,
	isSaveRequiredRoute
} from './shellRoutes';

describe('isSaveRequiredRoute', () => {
	it('matches save-required roots exactly', () => {
		expect(isSaveRequiredRoute('/edit')).toBe(true);
		expect(isSaveRequiredRoute('/registry')).toBe(true);
		expect(isSaveRequiredRoute('/gps')).toBe(true);
		expect(isSaveRequiredRoute('/ups')).toBe(true);
		expect(isSaveRequiredRoute('/blueprints')).toBe(true);
		expect(isSaveRequiredRoute('/debug')).toBe(true);
		expect(isSaveRequiredRoute('/servers')).toBe(true);
		expect(isSaveRequiredRoute('/overview')).toBe(true);
	});

	it('matches nested save-required routes', () => {
		expect(isSaveRequiredRoute('/edit/palbox')).toBe(true);
		expect(isSaveRequiredRoute('/edit/guild')).toBe(true);
		expect(isSaveRequiredRoute('/ups/anything/deep')).toBe(true);
	});

	it('treats save-agnostic routes as public', () => {
		expect(isSaveRequiredRoute('/')).toBe(false);
		expect(isSaveRequiredRoute('/map')).toBe(false);
		expect(isSaveRequiredRoute('/breeding')).toBe(false);
		expect(isSaveRequiredRoute('/about')).toBe(false);
		expect(isSaveRequiredRoute('/tools')).toBe(false);
		expect(isSaveRequiredRoute('/upload')).toBe(false);
		// The raw editor brings its own file; it needs no loaded save.
		expect(isSaveRequiredRoute('/editor')).toBe(false);
		expect(isSaveRequiredRoute('/docs')).toBe(false);
		expect(isSaveRequiredRoute('/docs/wiki/pals')).toBe(false);
	});

	it('does not match on bare string prefixes', () => {
		expect(isSaveRequiredRoute('/filesystem')).toBe(false);
		expect(isSaveRequiredRoute('/upsell')).toBe(false);
	});
});

describe('isFullBleedRoute', () => {
	it('treats the landing page and map as full-bleed', () => {
		expect(isFullBleedRoute('/')).toBe(true);
		expect(isFullBleedRoute('/map')).toBe(true);
	});

	it('does not treat content routes as full-bleed', () => {
		expect(isFullBleedRoute('/breeding')).toBe(false);
		expect(isFullBleedRoute('/about')).toBe(false);
		expect(isFullBleedRoute('/docs/wiki')).toBe(false);
		expect(isFullBleedRoute('/docs/wiki/pals')).toBe(false);
	});

	it('does not match every path against the root route', () => {
		expect(isFullBleedRoute('/tools')).toBe(false);
		expect(isFullBleedRoute('/upload')).toBe(false);
	});

	it('does not match on bare string prefixes', () => {
		expect(isFullBleedRoute('/mapping')).toBe(false);
	});
});

describe('isPublicShell', () => {
	it('is true only for the web build with no save loaded', () => {
		expect(isPublicShell(true, undefined)).toBe(true);
		expect(isPublicShell(true, null)).toBe(true);
	});

	it('is false once a save is loaded, on any build', () => {
		expect(isPublicShell(true, { name: 'Level.sav' })).toBe(false);
		expect(isPublicShell(false, { name: 'Level.sav' })).toBe(false);
	});

	it('is false on desktop even with no save, because the sidebar renders', () => {
		expect(isPublicShell(false, undefined)).toBe(false);
	});

	it('is false while remote mode routes this browser through a desktop', () => {
		expect(isPublicShell(true, undefined, true)).toBe(false);
		expect(isPublicShell(true, null, true)).toBe(false);
	});

	it('stays public on the web build when remote mode is off', () => {
		expect(isPublicShell(true, undefined, false)).toBe(true);
	});
});

describe('isCompatExemptRoute', () => {
	it('exempts the map, which browses without a save or filesystem access', () => {
		expect(isCompatExemptRoute('/map')).toBe(true);
	});

	it('keeps the notice on routes that do load or write a save', () => {
		expect(isCompatExemptRoute('/')).toBe(false);
		expect(isCompatExemptRoute('/upload')).toBe(false);
		expect(isCompatExemptRoute('/editor')).toBe(false);
		expect(isCompatExemptRoute('/edit/palbox')).toBe(false);
	});

	it('does not match on bare string prefixes', () => {
		expect(isCompatExemptRoute('/mapping')).toBe(false);
	});
});

describe('isPipWindow', () => {
	const url = (href: string) => new URL(href, 'http://127.0.0.1:5174');

	it('recognises the desktop pip window by its query flag', () => {
		expect(isPipWindow(url('/map?pip=1'))).toBe(true);
		expect(isPipWindow(url('/de/map?pip=1'))).toBe(true);
	});

	it('leaves the ordinary map window with its shell', () => {
		expect(isPipWindow(url('/map'))).toBe(false);
		expect(isPipWindow(url('/map?pip=0'))).toBe(false);
		expect(isPipWindow(url('/map?pip'))).toBe(false);
	});

	it('only strips the shell on the map', () => {
		expect(isPipWindow(url('/edit/palbox?pip=1'))).toBe(false);
		expect(isPipWindow(url('/?pip=1'))).toBe(false);
	});
});
