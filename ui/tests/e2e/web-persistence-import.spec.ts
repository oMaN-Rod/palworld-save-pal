import { test, expect } from '@playwright/test';
import { zipSync } from 'fflate';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

function world1Zip(): Buffer {
	const dir = resolve(__dirname, '../../../tests/fixtures/saves/world1');
	const entries: Record<string, Uint8Array> = {
		'Level.sav': new Uint8Array(readFileSync(resolve(dir, 'Level.sav')))
	};
	for (const p of readdirSync(resolve(dir, 'Players'))) {
		entries[`Players/${p}`] = new Uint8Array(readFileSync(resolve(dir, 'Players', p)));
	}
	return Buffer.from(zipSync(entries));
}

test('zip import persists and is offered as Resume after reload', async ({ page }) => {
	await page.goto('/upload');
	await page
		.locator('input[type=file][name=file]')
		.setInputFiles({ name: 'world1.zip', mimeType: 'application/zip', buffer: world1Zip() });
	await page.getByRole('button', { name: /^upload$/i }).click();
	await expect(page).toHaveURL(/\/edit/, { timeout: 30_000 });

	// Reload: the app boots to the web root/upload; a Resume control appears
	// because the imported save was persisted to OPFS.
	await page.goto('/upload');
	await expect(page.getByRole('button', { name: /^resume /i })).toBeVisible({ timeout: 15_000 });

	// Resuming re-loads the same world.
	await page.getByRole('button', { name: /^resume /i }).click();
	await expect(page).toHaveURL(/\/edit/, { timeout: 30_000 });
});
