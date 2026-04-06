import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { mdsvex } from 'mdsvex';
import rehypeExternalLinks from 'rehype-external-links';
import rehypeSlug from 'rehype-slug';
import { remarkTocHeadings } from './remark-toc-headings.js';

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
			pages: '../ui_build'
		}),
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
