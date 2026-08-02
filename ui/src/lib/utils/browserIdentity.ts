export type BrowserFamily = 'chromium' | 'firefox' | 'safari' | 'unknown';

export interface BrowserIdentity {
	family: BrowserFamily;
	name: string;
}

export interface NavigatorLike {
	userAgentData?: { brands?: { brand: string; version: string }[] };
	userAgent?: string;
}

const UNNAMED = 'this browser';

// Ordered most- to least-specific: Chrome's brand list also contains "Chromium",
// and we want the concrete product name for the banner headline.
const CHROMIUM_BRANDS = ['Microsoft Edge', 'Brave', 'Opera', 'Google Chrome', 'Chromium'];

export function detectBrowser(
	nav: NavigatorLike = typeof navigator === 'undefined' ? {} : (navigator as NavigatorLike)
): BrowserIdentity {
	// userAgentData is Chromium-only, so its mere presence settles the family.
	// Brand lists include deliberate GREASE entries ("Not_A Brand"), so match
	// against known names rather than taking brands[0].
	const brands = nav.userAgentData?.brands;
	if (brands?.length) {
		const known = CHROMIUM_BRANDS.find((b) => brands.some((x) => x.brand === b));
		return { family: 'chromium', name: known ?? UNNAMED };
	}

	const ua = nav.userAgent ?? '';
	// iOS Chrome (CriOS) and iOS Firefox (FxiOS) must be identified before
	// checking for /Safari\//. Both have Safari in their UA but run on WebKit,
	// so family is 'safari' (the engine that determines capabilities), not
	// 'chromium' or 'firefox'. The product name goes in the name field.
	if (/CriOS\//.test(ua)) return { family: 'safari', name: 'Chrome' };
	if (/FxiOS\//.test(ua)) return { family: 'safari', name: 'Firefox' };
	if (/Firefox\//.test(ua)) return { family: 'firefox', name: 'Firefox' };
	// Every Chromium UA also ends in "Safari/537.36", so Safari must be
	// identified by the ABSENCE of Chrome/Chromium.
	if (/Safari\//.test(ua) && !/Chrom(e|ium)\//.test(ua)) {
		return { family: 'safari', name: 'Safari' };
	}
	return { family: 'unknown', name: UNNAMED };
}
