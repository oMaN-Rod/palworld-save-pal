export const SAVE_REQUIRED_ROUTES = [
	'/edit',
	'/registry',
	'/gps',
	'/ups',
	'/blueprints',
	'/editor',
	'/debug',
	'/servers',
	'/overview'
] as const;

export function isSaveRequiredRoute(pathname: string): boolean {
	return SAVE_REQUIRED_ROUTES.some(
		(route) => pathname === route || pathname.startsWith(`${route}/`)
	);
}

export const FULL_BLEED_ROUTES = ['/', '/map'] as const;

export function isFullBleedRoute(pathname: string): boolean {
	return FULL_BLEED_ROUTES.some(
		(route) => pathname === route || (route !== '/' && pathname.startsWith(`${route}/`))
	);
}

export function isPublicShell(webBuild: boolean, saveFile: unknown): boolean {
	return webBuild && !saveFile;
}

// Browsing the map loads no save and touches no filesystem API, so the compat
// notice has nothing to warn about here. It also renders as a fixed, near
// full-width card that covers the map's own controls on a phone — the one place
// the map is explicitly meant to be used.
export const COMPAT_EXEMPT_ROUTES = ['/map'] as const;

export function isCompatExemptRoute(pathname: string): boolean {
	return COMPAT_EXEMPT_ROUTES.some(
		(route) => pathname === route || pathname.startsWith(`${route}/`)
	);
}
