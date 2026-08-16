import { paraglideVitePlugin } from '@inlang/paraglide-js';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import basicSsl from '@vitejs/plugin-basic-ssl';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { paraglideUrlPatterns } from './src/lib/i18n/routingConfig.js';

// Vite serves HTTP/2 only over TLS (resolveHttpServer hands https options to
// node:http2's createSecureServer), and browsers only negotiate h2 over TLS. Opt-in
// rather than always-on: the cert is self-signed, and psp-desktop/tauri.conf.json
// points its webview at http://localhost:5173, which WebView2 will not load over a
// self-signed origin.
const useHttps = process.env.VITE_HTTPS === '1';

export default defineConfig({
	plugins: [
		paraglideVitePlugin({
			project: './project.inlang',
			outdir: './src/paraglide',
			// `url` only matches the hub paths in routingConfig; editor routes have
			// no prefix and fall through to the persisted cookie setting.
			strategy: ['url', 'cookie', 'globalVariable', 'baseLocale'],
			urlPatterns: paraglideUrlPatterns
		}),
		tailwindcss(),
		sveltekit(),
		...(useHttps ? [basicSsl()] : [])
	],
	worker: {
		format: 'es'
	},
	optimizeDeps: {
		// Pre-bundling relocates the emscripten JS to .vite/deps, where its
		// locateFile lookup for the sibling sqlite3.wasm 404s. Keep it in its own
		// package folder so the wasm resolves in dev.
		exclude: ['@sqlite.org/sqlite-wasm'],
		// Each icon is its own deep-import entry, so lazy discovery re-runs the
		// optimizer once per route that introduces new ones. Every round swaps
		// .vite/deps through a temp dir; interrupt one and deps is left missing,
		// after which every pre-bundled import 504s and routes render as 500s.
		include: ['@lucide/svelte/icons/*']
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
	ssr: {
		// Skeleton's Zag packages pin mixed versions of @zag-js/core. Externalizing
		// them for SSR resolves every import to the single root install, which then
		// lacks exports the older packages expect. Bundling lets each resolve its own.
		noExternal: [/^@skeletonlabs\//, /^@zag-js\//]
	},
	test: {
		include: ['src/**/*.{test,spec}.{js,ts}', 'scripts/**/*.test.mjs', '../scripts/**/*.test.mjs']
	}
});
