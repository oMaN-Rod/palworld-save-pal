import { paraglideVitePlugin } from '@inlang/paraglide-js';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import basicSsl from '@vitejs/plugin-basic-ssl';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

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
			outdir: './src/paraglide'
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
		include: ['src/**/*.{test,spec}.{js,ts}', 'scripts/**/*.test.mjs', '../scripts/**/*.test.mjs']
	}
});
