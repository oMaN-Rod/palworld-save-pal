import { defineConfig, devices } from '@playwright/test';

const PHONE = { width: 390, height: 844 };
const TABLET = { width: 820, height: 1180 };
const DESKTOP = { width: 1440, height: 900 };

export default defineConfig({
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
	testIgnore: ['**/e2e/**', '**/mods/**'],
	use: { baseURL: 'http://localhost:4173' },
	projects: [
		{
			name: 'phone',
			testDir: 'tests/responsive',
			use: { ...devices['Desktop Chrome'], viewport: PHONE, hasTouch: true }
		},
		{
			name: 'tablet',
			testDir: 'tests/responsive',
			use: { ...devices['Desktop Chrome'], viewport: TABLET, hasTouch: true }
		},
		{
			name: 'desktop',
			testDir: 'tests/responsive',
			use: { ...devices['Desktop Chrome'], viewport: DESKTOP }
		},
		{
			name: 'app',
			testIgnore: ['**/e2e/**', '**/mods/**', '**/responsive/**'],
			use: { ...devices['Desktop Chrome'] }
		}
	]
});
