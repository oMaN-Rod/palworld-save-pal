import { test, expect } from '@playwright/test';
import { zipSync, unzipSync } from 'fflate';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

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

test('upload → edit-nothing → download round-trips a real world save', async ({ page }) => {
	await page.goto('/upload');

	await page
		.locator('input[type=file][name=file]')
		.setInputFiles({ name: 'world1.zip', mimeType: 'application/zip', buffer: world1Zip() });

	await page.getByRole('button', { name: /^upload$/i }).click();

	// Loading is a transient client-side route; the app lands on /edit once the
	// worker has parsed the save and player summaries have arrived.
	await expect(page).toHaveURL(/\/edit/, { timeout: 30_000 });

	// The download control lives on /upload's "current save" card; the nav
	// rail's upload/download tile is a plain <a href="/upload">, so this is a
	// client-side transition that keeps the in-memory save state.
	await page.locator('a[href="/upload"]').click();

	const [download] = await Promise.all([
		page.waitForEvent('download', { timeout: 30_000 }),
		page.getByRole('button', { name: /download/i }).click()
	]);

	const path = await download.path();
	expect(path).toBeTruthy();

	const zipBytes = new Uint8Array(readFileSync(path!));
	const files = unzipSync(zipBytes);
	const levelName = Object.keys(files).find((n) => n.endsWith('Level.sav'));
	expect(levelName).toBeTruthy();
	const level = files[levelName!];
	const magic = new TextDecoder().decode(level.subarray(0, 3));
	expect(magic).toBe('PlM');
});
