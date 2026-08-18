import { expect, test } from '@playwright/test';

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
	// Renders as an anchor with aria-label "Nexus Mods" (see landing/Link.svelte),
	// so it is a link, not a button, and the accessible name contains a space.
	await expect(page.getByRole('link', { name: /nexus\s*mods/i }).first()).toHaveAttribute(
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

	// No compatibility banner on a healthy desktop Chromium run. The unit tests
	// only prove detection matches hand-written fake scopes; this is the only
	// check that a real browser is not told it is broken.
	await expect(page.getByRole('status')).toHaveCount(0);
});

test('mobile landing hides save editing and shows the desktop-only notice', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/');

	// Title + desktop-only disclaimer
	await expect(page.getByRole('img', { name: /palworld save pal/i })).toBeVisible({
		timeout: 15_000
	});
	await expect(page.getByText(/desktop browser/i)).toBeVisible();

	// Save editing is not offered on phones: no dropzone or resume button
	await expect(page.getByText(/drop your save here/i)).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Choose .zip', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Choose folder', exact: true })).toHaveCount(0);

	// Everything else stays: tagline and marketing sections remain visible
	await expect(
		page.getByText(/the free, open-source palworld save editor\. in your browser\./i)
	).toBeVisible();
	await expect(page.getByText(/built different, on purpose/i)).toBeVisible();

	// Resize guard must not blackout public pages on phones
	await expect(page.getByText('Window Too Small')).toHaveCount(0);

	// Maps, Wiki, Breeding stay reachable via the public nav. Under 640px the
	// nav is a standard full-width top bar (still inline links, not a dropdown).
	await expect(page.getByRole('link', { name: 'Map', exact: true })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Wiki', exact: true })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Breeding', exact: true })).toBeVisible();
});
