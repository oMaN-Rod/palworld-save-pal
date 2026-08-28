import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { mdsvex } from 'mdsvex';
import rehypeExternalLinks from 'rehype-external-links';
import rehypeSlug from 'rehype-slug';
import { remarkTocHeadings } from './remark-toc-headings.js';
import { LOCALIZED_PATHS, localizedPath, siteLocales } from './src/lib/i18n/routingConfig.js';
import { readIsDesktopBuild } from './scripts/ensure-desktop-env.mjs';

const localizedEntries = LOCALIZED_PATHS.flatMap((pathname) =>
	siteLocales.map((locale) => localizedPath(pathname, locale))
);

// build:desktop force-writes ui/.env before vite starts, so this reads the same
// source `$env/static/public` serves to the app.
const isDesktop = readIsDesktopBuild();

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
			entries: isDesktop ? ['*'] : ['*', ...localizedEntries],
			// The category index renders every entity link so crawlers find them,
			// which means crawling would re-discover all 5,013 wiki entity pages no
			// matter what their entries() returns. Desktop keeps the indexes and
			// reaches the entity pages through the adapter fallback instead.
			crawl: !isDesktop
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
