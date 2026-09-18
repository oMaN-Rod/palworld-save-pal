import type { PlaywrightTestConfig } from '@playwright/test';

const config: PlaywrightTestConfig = {
	webServer: {
		command: 'npm run build && npm run preview',
		port: 4173,
		// A cold `vite build` runs well past Playwright's 60s default, which
		// failed the suite before it opened a page.
		timeout: 240_000,
		reuseExistingServer: !process.env.CI
	},
	testDir: 'tests',
	testMatch: /(.+\.)?(test|spec)\.[jt]s/,
	testIgnore: ['**/e2e/**', '**/mods/**']
};

export default config;
