import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { mdsvex } from 'mdsvex';
import rehypeExternalLinks from 'rehype-external-links';
import rehypeSlug from 'rehype-slug';
import { remarkTocHeadings } from './remark-toc-headings.js';
import { LOCALIZED_PATHS, localizedPath, siteLocales } from './src/lib/i18n/routingConfig.js';

const localizedEntries = LOCALIZED_PATHS.flatMap((pathname) =>
	siteLocales.map((locale) => localizedPath(pathname, locale))
);

/** @type {import('@sveltejs/kit').Config} */
const config = {
	extensions: ['.svelte', '.svx', '.md'],
	preprocess: [
		vitePreprocess(),
		mdsvex({
			extensions: ['.svx', '.md'],
			remarkPlugins: [remarkTocHeadings],
			rehypePlugins: [
				rehypeSlug,
				[rehypeExternalLinks, { target: '_blank', rel: ['noopener', 'noreferrer'] }]
			]
		})
	],

	kit: {
		adapter: adapter({
			pages: '../ui_build',
			// Paired with not_found_handling: "404-page" in wrangler.jsonc, so an
			// unknown URL returns HTTP 404 instead of a 200 soft-404 shell.
			fallback: '404.html'
		}),
		prerender: {
			entries: ['*', ...localizedEntries]
		},
		alias: {
			$theme: 'src/lib/theme',
			$components: 'src/lib/components',
			$ws: 'src/lib/ws',
			$types: 'src/lib/types',
			$states: 'src/lib/states',
			$utils: 'src/lib/utils',
			$i18n: 'src/paraglide',
			$docs: 'src/lib/docs'
		}
	}
};

export default config;
