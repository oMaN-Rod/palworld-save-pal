import { describe, it, expect, afterEach, vi } from 'vitest';
import { detectBrowser, type NavigatorLike } from './browserIdentity';

const CHROME_UA =
	'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36';
const SAFARI_UA =
	'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.3 Safari/605.1.15';
const FIREFOX_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:134.0) Gecko/20100101 Firefox/134.0';
const IOS_CHROME_UA =
	'Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/122.0.6261.62 Mobile/15E148 Safari/604.1';
const IOS_FIREFOX_UA =
	'Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/124.0 Mobile/15E148 Safari/605.1.15';

afterEach(() => vi.unstubAllGlobals());

describe('detectBrowser via userAgentData', () => {
	it('identifies Chrome and ignores GREASE brands', () => {
		const nav: NavigatorLike = {
			userAgentData: {
				brands: [
					{ brand: 'Not_A Brand', version: '８' },
					{ brand: 'Chromium', version: '133' },
					{ brand: 'Google Chrome', version: '133' }
				]
			}
		};
		expect(detectBrowser(nav)).toEqual({ family: 'chromium', name: 'Google Chrome' });
	});

	it('identifies Edge', () => {
		const nav: NavigatorLike = {
			userAgentData: { brands: [{ brand: 'Microsoft Edge', version: '133' }] }
		};
		expect(detectBrowser(nav)).toEqual({ family: 'chromium', name: 'Microsoft Edge' });
	});

	it('still reports chromium for an unrecognised brand list', () => {
		const nav: NavigatorLike = {
			userAgentData: { brands: [{ brand: 'Not_A Brand', version: '８' }] }
		};
		expect(detectBrowser(nav)).toEqual({ family: 'chromium', name: 'this browser' });
	});
});

describe('detectBrowser via user agent string', () => {
	it('identifies Firefox', () => {
		expect(detectBrowser({ userAgent: FIREFOX_UA })).toEqual({
			family: 'firefox',
			name: 'Firefox'
		});
	});

	it('identifies Safari', () => {
		expect(detectBrowser({ userAgent: SAFARI_UA })).toEqual({ family: 'safari', name: 'Safari' });
	});

	it('does NOT mistake Chrome for Safari', () => {
		// Chrome's UA also ends in "Safari/537.36" — the classic false positive.
		expect(detectBrowser({ userAgent: CHROME_UA }).family).not.toBe('safari');
	});

	it('falls back to unknown for an unrecognised agent', () => {
		expect(detectBrowser({ userAgent: 'SomeBot/1.0' })).toEqual({
			family: 'unknown',
			name: 'this browser'
		});
	});

	it('falls back to unknown when nothing is available', () => {
		expect(detectBrowser({})).toEqual({ family: 'unknown', name: 'this browser' });
	});

	it('identifies iOS Chrome with correct family and name', () => {
		// iOS forces all browsers onto WebKit, so family is 'safari' (the engine).
		// name is 'Chrome' (the product), for the display headline.
		expect(detectBrowser({ userAgent: IOS_CHROME_UA })).toEqual({
			family: 'safari',
			name: 'Chrome'
		});
	});

	it('identifies iOS Firefox with correct family and name', () => {
		// iOS forces all browsers onto WebKit, so family is 'safari' (the engine).
		// name is 'Firefox' (the product), for the display headline.
		expect(detectBrowser({ userAgent: IOS_FIREFOX_UA })).toEqual({
			family: 'safari',
			name: 'Firefox'
		});
	});

	it('does not throw when navigator is absent entirely', () => {
		// adapter-static prerenders in Node, where `navigator` may not exist —
		// this is the path the default parameter guards.
		vi.stubGlobal('navigator', undefined);
		expect(() => detectBrowser()).not.toThrow();
		expect(detectBrowser()).toEqual({ family: 'unknown', name: 'this browser' });
	});
});
