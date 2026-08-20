import { paraglideVitePlugin } from '@inlang/paraglide-js';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import basicSsl from '@vitejs/plugin-basic-ssl';
import { cpSync, existsSync, rmSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig, type Plugin } from 'vitest/config';
import { paraglideUrlPatterns } from './src/lib/i18n/routingConfig.js';

// Vite serves HTTP/2 only over TLS (resolveHttpServer hands https options to
// node:http2's createSecureServer), and browsers only negotiate h2 over TLS. Opt-in
// rather than always-on: the cert is self-signed, and psp-desktop/tauri.conf.json
// points its webview at http://localhost:5173, which WebView2 will not load over a
// self-signed origin.
const useHttps = process.env.VITE_HTTPS === '1';

// Self-host Monaco: @monaco-editor/loader defaults to pulling the editor from
// jsdelivr at runtime, which breaks the offline desktop app and adds a
// multi-MB CDN round-trip. The copy lands in static/vs (gitignored, rebuilt
// from node_modules) so dev, desktop, and the webapp all serve '/vs'.
function selfHostMonaco(): Plugin {
	const src = fileURLToPath(new URL('./node_modules/monaco-editor/min/vs', import.meta.url));
	const dest = fileURLToPath(new URL('./static/vs', import.meta.url));
	return {
		name: 'psp:self-host-monaco',
		buildStart() {
			if (!existsSync(src)) return;
			const srcTime = statSync(src).mtimeMs;
			const destTime = existsSync(dest) ? statSync(dest).mtimeMs : 0;
			if (destTime >= srcTime) return;
			rmSync(dest, { recursive: true, force: true });
			cpSync(src, dest, { recursive: true });
		}
	};
}

export default defineConfig({
	plugins: [
		paraglideVitePlugin({
			project: './project.inlang',
			outdir: './src/paraglide',
			// `url` strategy only matches the hub paths in routingConfig; editor
			// routes have no prefix and fall through to the persisted cookie setting.
			strategy: ['url', 'cookie', 'globalVariable', 'baseLocale'],
			urlPatterns: paraglideUrlPatterns
		}),
		tailwindcss(),
		sveltekit(),
		selfHostMonaco(),
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
	ssr: {
		// Skeleton's Zag packages pin mixed versions of @zag-js/core. Externalizing
		// them for SSR resolves every import to the single root install, which then
		// lacks exports the older packages expect. Bundling lets each resolve its own.
		noExternal: [/^@skeletonlabs\//, /^@zag-js\//, 'maplibre-gl']
	},
	test: {
		include: ['src/**/*.{test,spec}.{js,ts}', 'scripts/**/*.test.mjs', '../scripts/**/*.test.mjs']
	}
});
