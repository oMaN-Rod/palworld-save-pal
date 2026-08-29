import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
// A PlM/Oodle container, the shape the game actually writes. wasm32 links no
// Oodle codec, so this is the file that proves the browser engine is using the
// worker's lent one -- a PlZ/zlib save would pass without it.
const FIXTURE = '43797F87000000000000000000000000.sav';

function playerSav(): Buffer {
	return Buffer.from(
		readFileSync(resolve(here, '../../../tests/fixtures/saves/world1/Players', FIXTURE))
	);
}

// The browser build ships no backend, so the editor's conversion has to happen
// in the worker's own engine. Reaching /editor without a loaded save is half the
// test: the public shell used to bounce it to /upload.
test('the raw editor converts a .sav with no save loaded and no backend', async ({ page }) => {
	// The first conversion waits on the wasm module, its migrations and the game
	// data, which is well past the default per-test budget.
	test.setTimeout(180_000);

	await page.goto('/editor');
	await expect(page).toHaveURL(/\/editor/);

	await page.locator('input[type=file][accept=".sav"]').setInputFiles({
		name: FIXTURE,
		mimeType: 'application/octet-stream',
		buffer: playerSav()
	});

	// The toolbar renders only once the JSON is in the editor.
	await expect(page.getByRole('button', { name: 'Save' })).toBeVisible({ timeout: 90_000 });
	await expect(page.getByRole('button', { name: 'Format' })).toBeVisible();

	const [download] = await Promise.all([
		page.waitForEvent('download', { timeout: 60_000 }),
		page.getByRole('button', { name: 'Save' }).click()
	]);

	const bytes = new Uint8Array(readFileSync((await download.path())!));
	// The container magic sits at byte 8, after the two length words.
	expect(new TextDecoder().decode(bytes.subarray(8, 11))).toBe('PlM');
});

test('the public shell links to the raw editor', async ({ page }) => {
	await page.goto('/map');
	await expect(page.locator('nav a[href="/editor"]')).toBeVisible();
});
