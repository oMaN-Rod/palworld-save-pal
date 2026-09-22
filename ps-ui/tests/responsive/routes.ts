/**
 * Routes that render on their own in a desktop build with no save loaded. The
 * save-required routes still render their empty state here — in a web build
 * they would redirect to `/upload` instead, which is why this suite runs
 * against the desktop build.
 */
export const CORE_ROUTES = [
	'/',
	'/map',
	'/edit/player',
	'/edit/palbox',
	'/edit/guild',
	'/edit/technologies',
	'/gps',
	'/ups',
	'/live'
] as const;
