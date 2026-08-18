import { env } from '$env/dynamic/public';
import { paraglideMiddleware } from '$i18n/server';
import { htmlLanguageTags } from '$lib/i18n/routingConfig.js';
import type { Handle } from '@sveltejs/kit';

// Runs at prerender time under adapter-static, which is what substitutes
// %lang%/%dir% into the static HTML. Without it the placeholders ship literally.
const BEACON_BLOCK = /<!--CF_BEACON_START-->[\s\S]*?<!--CF_BEACON_END-->/;

/**
 * Drop the analytics beacon entirely rather than shipping a dead request.
 * The block is collapsed to placeholder comments instead of being deleted:
 * SvelteKit warns (and hydration can break) when a page-chunk transform
 * removes HTML comments, so keep the same number of comment nodes present.
 */
function applyBeacon(html: string): string {
	const token = env.PUBLIC_CF_BEACON_TOKEN;
	if (!token) {
		return html.replace(
			BEACON_BLOCK,
			'<!--CF_BEACON_START--><!--CF_BEACON_STRIPPED--><!--CF_BEACON_END-->'
		);
	}
	return html.replace('%CF_BEACON_TOKEN%', token);
}

export const handle: Handle = ({ event, resolve }) =>
	paraglideMiddleware(event.request, ({ request, locale }) => {
		event.request = request;
		const lang = htmlLanguageTags[locale as keyof typeof htmlLanguageTags] ?? locale;
		return resolve(event, {
			transformPageChunk: ({ html }) =>
				applyBeacon(html.replace('%lang%', lang).replace('%dir%', 'ltr'))
		});
	});
