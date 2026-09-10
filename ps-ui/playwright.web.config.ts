import { defineConfig } from '@playwright/test';

export default defineConfig({
	testDir: 'tests/e2e',
	webServer: {
		command: 'bun run build:web && bun run preview --port 4173',
		port: 4173,
		reuseExistingServer: !process.env.CI,
		timeout: 240_000
	},
	use: { baseURL: 'http://localhost:4173' }
});
