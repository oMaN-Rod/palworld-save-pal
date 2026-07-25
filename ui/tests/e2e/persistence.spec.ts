import { test, expect } from '@playwright/test';

test('a settings write survives a reload (OPFS persistence)', async ({ page }) => {
	test.setTimeout(60_000);

	const clonePrefix = `e2e-${Date.now()}`;

	await page.goto('/upload');

	// Bootstrap resets a dozen+ in-memory data stores over the worker
	// transport before the settings sync roundtrip lands. Opening Settings
	// before it lands would bind the modal to a settings object that
	// bootstrap is about to replace wholesale, silently dropping our edit.
	await page.waitForTimeout(2000);

	await page.getByTitle('Settings', { exact: true }).click();
	await page.getByLabel('Clone Prefix').fill(clonePrefix);
	await page.locator('[data-modal-primary]').click();

	// Saving settings writes to the sqlite-in-wasm/OPFS DB, then the app
	// schedules its own reload. Wait for that reload rather than racing it.
	await page.waitForEvent('load', { timeout: 15_000 });

	// A second, explicit reload is the actual proof: the value must come back
	// from a fresh DB read (OPFS), not from anything still held in memory.
	await page.reload();
	await page.waitForTimeout(2000);

	await page.getByTitle('Settings', { exact: true }).click();
	await expect(page.getByLabel('Clone Prefix')).toHaveValue(clonePrefix, { timeout: 15_000 });
});
