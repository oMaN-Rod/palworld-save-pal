import { expect, test } from '@playwright/test';

const TABLET_PORTRAIT = { width: 820, height: 1180 };
const TABLET_LANDSCAPE = { width: 1180, height: 820 };

// Rotation crosses a breakpoint; state in a component that unmounts is lost silently.
test.describe('rotation', () => {
	// The preview build has no backend, so `/edit/*` is only its no-save empty state.

	test('keeps the map options sheet open across a rotation', async ({ page }) => {
		await page.setViewportSize({ width: 390, height: 844 });
		await page.goto('/map');
		await page.waitForLoadState('networkidle');

		await page.getByRole('button', { name: 'Map Options' }).click();
		const sheet = page.getByRole('dialog', { name: 'Map Options' });
		await expect(sheet).toBeVisible();

		await page.setViewportSize({ width: 844, height: 390 });
		await page.waitForTimeout(300);

		// Past `md` the options are a side panel rather than a sheet.
		await expect(page.getByRole('dialog', { name: 'Map Options' })).toBeHidden();
		await expect(page.getByText('Map Options').first()).toBeVisible();
	});

	test('does not scroll sideways in either orientation', async ({ page }) => {
		for (const size of [TABLET_PORTRAIT, TABLET_LANDSCAPE]) {
			await page.setViewportSize(size);
			await page.goto('/edit/player');
			await page.waitForLoadState('networkidle');

			const { scrollWidth, clientWidth } = await page.evaluate(() => {
				const el = document.scrollingElement as HTMLElement;
				return { scrollWidth: el.scrollWidth, clientWidth: el.clientWidth };
			});
			expect(scrollWidth, `${size.width}x${size.height}`).toBeLessThanOrEqual(clientWidth);
		}
	});
});
