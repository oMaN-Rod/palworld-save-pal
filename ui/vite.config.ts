import { fileURLToPath } from 'node:url';
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
	worker: {
		format: 'es'
	},
	optimizeDeps: {
		// Pre-bundling relocates the emscripten JS to .vite/deps, where its
		// locateFile lookup for the sibling sqlite3.wasm 404s. Keep it in its own
		// package folder so the wasm resolves in dev.
		exclude: ['@sqlite.org/sqlite-wasm']
	},
	server: {
		// tauri.conf.json devUrl points here; fail loudly rather than drifting to
		// another port and leaving the desktop webview on a dead URL.
		port: 5173,
		strictPort: true,
		// The Oodle (ooz) wasm module lives in ui/vendor, outside Vite's default dev
		// fs sandbox; allow it so the web worker can load ooz.mjs/ooz.wasm in dev.
		fs: {
			allow: [fileURLToPath(new URL('./vendor', import.meta.url))]
		},
		proxy: {
			'/api': {
				target: 'http://localhost:5174',
				changeOrigin: true
			}
		}
	},
	test: {
		include: [
			'src/**/*.{test,spec}.{js,ts}',
			'scripts/**/*.test.mjs',
			'../scripts/**/*.test.mjs'
		]
	}
});
