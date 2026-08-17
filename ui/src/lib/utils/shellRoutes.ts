// Routes that are meaningless without a loaded save. A public-shell visitor
// landing on one is redirected home rather than shown a broken page.
export const SAVE_REQUIRED_ROUTES = [
	'/edit',
	'/bulk',
	'/gps',
	'/ups',
	'/blueprints',
	'/editor',
	'/debug',
	'/servers',
	'/file'
] as const;

export function isSaveRequiredRoute(pathname: string): boolean {
	return SAVE_REQUIRED_ROUTES.some(
		(route) => pathname === route || pathname.startsWith(`${route}/`)
	);
}

// Routes that paint their own full-bleed surface to the top of the viewport.
// The public shell's floating nav overlays these instead of reserving a band
// above them.
export const FULL_BLEED_ROUTES = ['/', '/map'] as const;

export function isFullBleedRoute(pathname: string): boolean {
	return FULL_BLEED_ROUTES.some(
		(route) => pathname === route || (route !== '/' && pathname.startsWith(`${route}/`))
	);
}

// Whether the floating public nav pill is on screen. It is fixed to the top
// centre, so anything else that wants that spot has to clear it. Distinct from
// "no save is loaded": the desktop build renders the sidebar instead, leaving
// the top centre free.
export function isPublicShell(webBuild: boolean, saveFile: unknown): boolean {
	return webBuild && !saveFile;
}
