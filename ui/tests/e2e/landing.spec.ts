import { test, expect } from '@playwright/test';

test('web root shows the redesigned landing with the save dropzone', async ({ page }) => {
	await page.goto('/');
	// Theme logo
	await expect(page.getByRole('img', { name: /palworld save pal/i })).toBeVisible({
		timeout: 15_000
	});
	// Unified dropzone with both browse buttons
	await expect(page.getByText(/drop your save here/i)).toBeVisible();
	await expect(page.getByRole('button', { name: 'Choose .zip', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Choose folder', exact: true })).toBeVisible();
	// Featured 3D map claim
	await expect(
		page.getByText(/first palworld save editor with a full 3d world map/i)
	).toBeVisible();
	// No sidebar/nav on the landing (NavBar renders Skeleton's NavRail, data-testid="nav-rail")
	await expect(page.locator('[data-testid="nav-rail"]')).toHaveCount(0);
});
