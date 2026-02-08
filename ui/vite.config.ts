import { paraglideVitePlugin } from '@inlang/paraglide-js';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [
		paraglideVitePlugin({
			project: './project.inlang',
			outdir: './src/paraglide'
		}),
		tailwindcss(),
		sveltekit()
	],
	test: {
		include: ['src/**/*.{test,spec}.{js,ts}', 'static/**/*.{png,json}']
	}
});
