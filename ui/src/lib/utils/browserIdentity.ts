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

// Ordered most- to least-specific: Chrome's brand list also contains "Chromium",
// and we want the concrete product name for the banner headline.
const CHROMIUM_BRANDS = ['Microsoft Edge', 'Brave', 'Opera', 'Google Chrome', 'Chromium'];

// iPadOS 13+ reports a desktop Macintosh user agent with no mobile token at
// all, so the only signal left is that real Macs are not multi-touch. This is a
// heuristic: a Mac with a connected touch display would be misread as an iPad.
function isMobile(nav: NavigatorLike, ua: string): boolean {
	if (/Android|iPhone|iPad|iPod|CriOS\/|FxiOS\//.test(ua)) return true;
	return /Macintosh/.test(ua) && (nav.maxTouchPoints ?? 0) > 1;
}

export function detectBrowser(
	nav: NavigatorLike = typeof navigator === 'undefined' ? {} : (navigator as NavigatorLike)
): BrowserIdentity {
	const ua = nav.userAgent ?? '';
	const mobile = isMobile(nav, ua);

	// userAgentData is Chromium-only, so its mere presence settles the family.
	// Brand lists include deliberate GREASE entries ("Not_A Brand"), so match
	// against known names rather than taking brands[0].
	const brands = nav.userAgentData?.brands;
	if (brands?.length) {
		const known = CHROMIUM_BRANDS.find((b) => brands.some((x) => x.brand === b));
		return { family: 'chromium', name: known ?? UNNAMED, mobile };
	}

	// iOS Chrome (CriOS) and iOS Firefox (FxiOS) must be identified before
	// checking for /Safari\//. Both have Safari in their UA but run on WebKit,
	// so family is 'safari' (the engine that determines capabilities), not
	// 'chromium' or 'firefox'. The product name goes in the name field.
	if (/CriOS\//.test(ua)) return { family: 'safari', name: 'Chrome', mobile };
	if (/FxiOS\//.test(ua)) return { family: 'safari', name: 'Firefox', mobile };
	if (/Firefox\//.test(ua)) return { family: 'firefox', name: 'Firefox', mobile };
	// Every Chromium UA also ends in "Safari/537.36", so Safari must be
	// identified by the ABSENCE of Chrome/Chromium.
	if (/Safari\//.test(ua) && !/Chrom(e|ium)\//.test(ua)) {
		return { family: 'safari', name: 'Safari', mobile };
	}
	return { family: 'unknown', name: UNNAMED, mobile };
}
