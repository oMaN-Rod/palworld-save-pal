import { paraglideMiddleware } from '$i18n/server';
import { htmlLanguageTags } from '$lib/i18n/routingConfig.js';
import type { Handle } from '@sveltejs/kit';

// Runs at prerender time under adapter-static, which is what substitutes
// %lang%/%dir% into the static HTML. Without it the placeholders ship literally.
export const handle: Handle = ({ event, resolve }) =>
	paraglideMiddleware(event.request, ({ request, locale }) => {
		event.request = request;
		const lang = htmlLanguageTags[locale as keyof typeof htmlLanguageTags] ?? locale;
		return resolve(event, {
			transformPageChunk: ({ html }) => html.replace('%lang%', lang).replace('%dir%', 'ltr')
		});
	});
