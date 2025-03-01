import { skeleton } from '@skeletonlabs/skeleton/plugin';
import * as themes from '@skeletonlabs/skeleton/themes';
import { join } from 'path';
import type { Config } from 'tailwindcss';
import psp from './src/lib/theme/psp';

export default {
	darkMode: 'selector',
	content: [
		'./src/**/*.{html,js,svelte,ts}',
		join(require.resolve('@skeletonlabs/skeleton-svelte'), '../**/*.{html,js,svelte,ts}')
	],

	theme: {
		extend: {
			spacing: {
				'18': '72px'
			},
			colors: {
				'tech-bg': '#001a1a',
				'tech-border': '#00ffff',
				'ancient-bg': '#1a001a',
				'ancient-border': '#ff00ff'
			}
		}
	},

	plugins: [
		skeleton({
			themes: [themes.cerberus, themes.rose, psp]
		})
	]
} as Config;
