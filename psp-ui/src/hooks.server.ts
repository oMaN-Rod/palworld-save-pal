import { env } from '$env/dynamic/public';
import { paraglideMiddleware } from '$i18n/server';
import { htmlLanguageTags } from '$lib/i18n/routingConfig.js';
import type { Handle } from '@sveltejs/kit';

// Runs at prerender time under adapter-static, which is what substitutes
// %lang%/%dir% into the static HTML. Without it the placeholders ship literally.
//
// The CF beacon block is dropped from the template at config load when no token
// is set (see svelte.config.js), so this only substitutes placeholders and never
// removes an HTML comment — Svelte's hydration anchors stay intact.
function applyBeacon(html: string): string {
	const token = env.PUBLIC_CF_BEACON_TOKEN;
	if (!token) return html;
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
