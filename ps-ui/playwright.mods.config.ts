import { defineConfig } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Playwright re-evaluates the config in each worker; workers inherit the runner's root and port.
let appRoot = process.env.PS_MODS_E2E_ROOT;
if (!appRoot) {
	const root = mkdtempSync(join(tmpdir(), 'ps-mods-e2e-'));
	appRoot = root;
	process.env.PS_MODS_E2E_ROOT = root;
	process.on('exit', () => {
		try {
			rmSync(root, { recursive: true, force: true, maxRetries: 5 });
		} catch {
			// Best effort: a lingering file lock leaves the directory in the temp folder.
		}
	});
}
const db = join(appRoot, 'ps-rs.db');

// The config loads synchronously, so a child process asks the OS for a free port.
let port = process.env.PS_MODS_E2E_PORT;
if (!port) {
	port = execFileSync(
		process.execPath,
		[
			'-e',
			"const s = require('node:net').createServer(); s.listen(0, '127.0.0.1', () => { process.stdout.write(String(s.address().port)); s.close(); });"
		],
		{ encoding: 'utf8' }
	).trim();
	process.env.PS_MODS_E2E_PORT = port;
}

export default defineConfig({
	testDir: 'tests/mods',
	fullyParallel: false,
	workers: 1,
	retries: 0,
	timeout: 180_000,
	expect: { timeout: 30_000 },
	webServer: {
		command: `bunx vite build && cargo run -p ps-server -- --host 127.0.0.1 --port ${port} --data-dir ../data --ui-dir ../ui_build --db "${db}"`,
		url: `http://127.0.0.1:${port}`,
		env: {
			PUBLIC_WS_URL: `127.0.0.1:${port}/ws`,
			PUBLIC_DESKTOP_MODE: 'false',
			PS_APP_ROOT: appRoot
		},
		reuseExistingServer: false,
		timeout: 1_200_000
	},
	use: { baseURL: `http://127.0.0.1:${port}` }
});
