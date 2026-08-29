// Writes psp-ui/.env for desktop builds. Default mode repairs the file only when it
// is absent or not a usable desktop env, so `cargo tauri dev` works on a fresh
// clone and after a `build:web` run (which force-writes the web values into the
// same file) without stomping a customized desktop env. `--force` overwrites
// unconditionally, which the desktop *build* (`build:desktop`) uses so a release
// can never bake a stale/web-mode value. .env is gitignored and generated
// everywhere (CI, Dockerfile, scripts/build-ui-desktop.*).
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const DESKTOP_ENV = 'PUBLIC_WS_URL=127.0.0.1:5174/ws\nPUBLIC_DESKTOP_MODE=true\n';

export const ENV_PATH = join(dirname(dirname(fileURLToPath(import.meta.url))), '.env');

function parseEnv(contents) {
	const vars = {};
	for (const line of contents.split('\n')) {
		const trimmed = line.trim();
		if (!trimmed || trimmed.startsWith('#')) continue;
		const eq = trimmed.indexOf('=');
		if (eq === -1) continue;
		vars[trimmed.slice(0, eq).trim()] = trimmed.slice(eq + 1).trim();
	}
	return vars;
}

/**
 * @param {string | null} existing contents of psp-ui/.env, or null when absent
 * @param {{ force?: boolean }} [options]
 */
export function desktopEnvNeedsWrite(existing, { force = false } = {}) {
	if (force || existing === null) return true;
	const vars = parseEnv(existing);
	// An empty PUBLIC_WS_URL makes the webview build `ws:///<clientId>`, which
	// fails URL parsing — the socket never opens and every send() hangs.
	return vars.PUBLIC_DESKTOP_MODE !== 'true' || !vars.PUBLIC_WS_URL;
}

/**
 * The build target, read from the same psp-ui/.env that `$env/static/public` serves
 * to the app. A missing file means web, so a fresh clone still produces the full
 * SEO output rather than silently shipping a desktop-shaped build.
 *
 * @param {string | null} contents contents of psp-ui/.env, or null when absent
 */
export function isDesktopEnv(contents) {
	if (contents === null) return false;
	return parseEnv(contents).PUBLIC_DESKTOP_MODE === 'true';
}

/** @param {string} [envPath] */
export function readIsDesktopBuild(envPath = ENV_PATH) {
	return isDesktopEnv(existsSync(envPath) ? readFileSync(envPath, 'utf8') : null);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	const force = process.argv.includes('--force');
	const envPath = ENV_PATH;
	const existing = existsSync(envPath) ? readFileSync(envPath, 'utf8') : null;

	if (desktopEnvNeedsWrite(existing, { force })) {
		writeFileSync(envPath, DESKTOP_ENV);
		console.log(`[ensure-desktop-env] wrote psp-ui/.env for desktop${force ? ' (forced)' : ''}`);
	} else {
		console.log('[ensure-desktop-env] psp-ui/.env already set for desktop, leaving it alone');
	}
}
