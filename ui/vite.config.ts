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
	server: {
		// tauri.conf.json devUrl points here; fail loudly rather than drifting to
		// another port and leaving the desktop webview on a dead URL.
		port: 5173,
		strictPort: true,
		proxy: {
			'/api': {
				target: 'http://localhost:5174',
				changeOrigin: true
			}
		}
	},
	test: {
		include: ['src/**/*.{test,spec}.{js,ts}']
	}
});
