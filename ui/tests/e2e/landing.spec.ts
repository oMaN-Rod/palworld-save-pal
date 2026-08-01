import { test, expect } from '@playwright/test';

test('web root shows the landing with an open-your-save CTA', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('heading', { name: /edit your palworld saves/i })).toBeVisible({
		timeout: 15_000
	});
	const open = page.getByRole('button', { name: /open your save/i });
	await expect(open).toBeVisible();
	await open.click();
	await expect(page).toHaveURL(/\/upload/, { timeout: 15_000 });
});
