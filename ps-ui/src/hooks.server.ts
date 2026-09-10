import { env } from '$env/dynamic/public';
import { paraglideMiddleware } from '$i18n/server';
import { htmlLanguageTags, localizedPath, siteLocales } from '$lib/i18n/routingConfig.js';
import type { Handle } from '@sveltejs/kit';

// Runs at prerender time under adapter-static, which is what substitutes
// %lang%/%dir% into the static HTML. Without it the placeholders ship literally.
const BEACON_BLOCK = /<!--CF_BEACON_START-->[\s\S]*?<!--CF_BEACON_END-->/;

// The pairing secret must never reach a third-party script on this route, so
// the beacon is stripped here regardless of token, for every locale prefix.
const SIGNAL_PATHS = new Set(siteLocales.map((locale) => localizedPath('/signal', locale)));

/** Drop the analytics beacon entirely rather than shipping a dead request. */
function applyBeacon(html: string, pathname: string): string {
	if (SIGNAL_PATHS.has(pathname)) return html.replace(BEACON_BLOCK, '');
	const token = env.PUBLIC_CF_BEACON_TOKEN;
	if (!token) return html.replace(BEACON_BLOCK, '');
	return html.replace('%CF_BEACON_TOKEN%', token);
}

export const handle: Handle = ({ event, resolve }) =>
	paraglideMiddleware(event.request, ({ request, locale }) => {
		event.request = request;
		const lang = htmlLanguageTags[locale as keyof typeof htmlLanguageTags] ?? locale;
		return resolve(event, {
			transformPageChunk: ({ html }) =>
				applyBeacon(html.replace('%lang%', lang).replace('%dir%', 'ltr'), event.url.pathname)
		});
	});
