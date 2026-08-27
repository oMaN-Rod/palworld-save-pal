export const SAVE_REQUIRED_ROUTES = [
	'/edit',
	'/bulk',
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
