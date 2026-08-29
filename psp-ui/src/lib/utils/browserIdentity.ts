export type BrowserFamily = 'chromium' | 'firefox' | 'safari' | 'unknown';

export interface BrowserIdentity {
	family: BrowserFamily;
	name: string;
	mobile: boolean;
}

export interface NavigatorLike {
	userAgentData?: { brands?: { brand: string; version: string }[] };
	userAgent?: string;
	maxTouchPoints?: number;
}

const UNNAMED = 'this browser';

const CHROMIUM_BRANDS = ['Microsoft Edge', 'Brave', 'Opera', 'Google Chrome', 'Chromium'];

function isMobile(nav: NavigatorLike, ua: string): boolean {
	if (/Android|iPhone|iPad|iPod|CriOS\/|FxiOS\//.test(ua)) return true;
	return /Macintosh/.test(ua) && (nav.maxTouchPoints ?? 0) > 1;
}

export function detectBrowser(
	nav: NavigatorLike = typeof navigator === 'undefined' ? {} : (navigator as NavigatorLike)
): BrowserIdentity {
	const ua = nav.userAgent ?? '';
	const mobile = isMobile(nav, ua);

	const brands = nav.userAgentData?.brands;
	if (brands?.length) {
		const known = CHROMIUM_BRANDS.find((b) => brands.some((x) => x.brand === b));
		return { family: 'chromium', name: known ?? UNNAMED, mobile };
	}

	if (/CriOS\//.test(ua)) return { family: 'safari', name: 'Chrome', mobile };
	if (/FxiOS\//.test(ua)) return { family: 'safari', name: 'Firefox', mobile };
	if (/Firefox\//.test(ua)) return { family: 'firefox', name: 'Firefox', mobile };
	if (/Safari\//.test(ua) && !/Chrom(e|ium)\//.test(ua)) {
		return { family: 'safari', name: 'Safari', mobile };
	}
	return { family: 'unknown', name: UNNAMED, mobile };
}
