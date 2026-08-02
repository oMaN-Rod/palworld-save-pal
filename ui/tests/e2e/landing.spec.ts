import { test, expect } from '@playwright/test';

test('web landing shows the Adventure Atlas redesign', async ({ page }) => {
	await page.goto('/');

	// Theme logo
	await expect(page.getByRole('img', { name: /palworld save pal/i })).toBeVisible({
		timeout: 15_000
	});

	// Tagline
	await expect(
		page.getByText(/the free, open-source palworld save editor\. in your browser\./i)
	).toBeVisible();

	// Unified dropzone with both browse buttons
	await expect(page.getByText(/drop your save here/i)).toBeVisible();
	await expect(page.getByRole('button', { name: 'Choose .zip', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Choose folder', exact: true })).toBeVisible();

	// 3D map claim
	await expect(
		page.getByText(/first palworld save editor with a full 3d world map/i)
	).toBeVisible();

	// Section headings
	await expect(page.getByText(/built different, on purpose/i)).toBeVisible();
	await expect(page.getByText(/everything you can edit/i)).toBeVisible();
	await expect(page.getByText(/three steps/i)).toBeVisible();
	await expect(page.getByText(/prefer the desktop app\? it is here to stay/i)).toBeVisible();

	// FAQ accordion: an item expands on click
	const faqSummary = page.getByText(/do my files get uploaded anywhere\?/i);
	await faqSummary.click();
	await expect(page.getByText(/everything runs in your browser/i)).toBeVisible();

	// Desktop links
	await expect(page.getByRole('link', { name: /github/i }).first()).toHaveAttribute(
		'href',
		'https://github.com/oMaN-Rod/palworld-save-pal'
	);
	await expect(page.getByRole('button', { name: /nexusmods/i })).toHaveAttribute(
		'href',
		'https://www.nexusmods.com/palworld/mods/1827'
	);

	// Footer social links
	await expect(page.getByRole('link', { name: 'Discord' })).toHaveAttribute(
		'href',
		'https://discord.gg/YWZFPy9G8J'
	);

	// No sidebar/nav rail on the landing
	await expect(page.locator('.nav-rail')).toHaveCount(0);
});
