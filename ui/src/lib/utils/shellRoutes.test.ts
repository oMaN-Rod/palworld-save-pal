import { describe, it, expect } from 'vitest';
import { isSaveRequiredRoute, isFullBleedRoute } from './shellRoutes';

describe('isSaveRequiredRoute', () => {
	it('matches save-required roots exactly', () => {
		expect(isSaveRequiredRoute('/edit')).toBe(true);
		expect(isSaveRequiredRoute('/bulk')).toBe(true);
		expect(isSaveRequiredRoute('/gps')).toBe(true);
		expect(isSaveRequiredRoute('/ups')).toBe(true);
		expect(isSaveRequiredRoute('/blueprints')).toBe(true);
		expect(isSaveRequiredRoute('/editor')).toBe(true);
		expect(isSaveRequiredRoute('/debug')).toBe(true);
		expect(isSaveRequiredRoute('/servers')).toBe(true);
		expect(isSaveRequiredRoute('/file')).toBe(true);
	});

	it('matches nested save-required routes', () => {
		expect(isSaveRequiredRoute('/edit/palbox')).toBe(true);
		expect(isSaveRequiredRoute('/edit/guild')).toBe(true);
		expect(isSaveRequiredRoute('/ups/anything/deep')).toBe(true);
	});

	it('treats save-agnostic routes as public', () => {
		expect(isSaveRequiredRoute('/')).toBe(false);
		expect(isSaveRequiredRoute('/map')).toBe(false);
		expect(isSaveRequiredRoute('/worldmap')).toBe(false);
		expect(isSaveRequiredRoute('/breeding')).toBe(false);
		expect(isSaveRequiredRoute('/about')).toBe(false);
		expect(isSaveRequiredRoute('/tools')).toBe(false);
		expect(isSaveRequiredRoute('/upload')).toBe(false);
		expect(isSaveRequiredRoute('/docs')).toBe(false);
		expect(isSaveRequiredRoute('/docs/wiki/pals')).toBe(false);
	});

	it('does not match on bare string prefixes', () => {
		expect(isSaveRequiredRoute('/editorial')).toBe(false);
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
