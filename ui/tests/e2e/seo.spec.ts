import { expect, test } from '@playwright/test';

const ORIGIN = 'https://palworldsavepal.app';

test.describe('SEO head tags', () => {
	test('landing page carries canonical, description and h1', async ({ page }) => {
		await page.goto('/');
		await expect(page).toHaveTitle(/Palworld Save Editor/i);
		await expect(page.locator('link[rel="canonical"]')).toHaveAttribute('href', `${ORIGIN}/`);
		await expect(page.locator('meta[name="description"]')).toHaveAttribute(
			'content',
			/Palworld/i
		);
		await expect(page.locator('h1')).toHaveCount(1);
	});

	test('landing page advertises every locale exactly once', async ({ page }) => {
		await page.goto('/');
		// 16 locales + x-default
		await expect(page.locator('link[rel="alternate"]')).toHaveCount(17);
		await expect(page.locator('link[hreflang="x-default"]')).toHaveAttribute('href', `${ORIGIN}/`);
		await expect(page.locator('link[hreflang="zh-Hans"]')).toHaveAttribute('href', `${ORIGIN}/zh`);
	});

	test('localized page canonicalizes to itself', async ({ page }) => {
		await page.goto('/fr');
		await expect(page.locator('link[rel="canonical"]')).toHaveAttribute('href', `${ORIGIN}/fr`);
		await expect(page.locator('html')).toHaveAttribute('lang', 'fr');
	});

	test('wiki entity page is English-only with no alternates', async ({ page }) => {
		await page.goto('/wiki/pals/sheepball');
		await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
			'href',
			`${ORIGIN}/wiki/pals/sheepball`
		);
		await expect(page.locator('link[rel="alternate"]')).toHaveCount(0);
	});

	test('landing page emits parseable structured data', async ({ page }) => {
		await page.goto('/');
		const blocks = await page.locator('script[type="application/ld+json"]').allTextContents();
		expect(blocks.length).toBeGreaterThan(0);
		const types = blocks.flatMap((block) => {
			const parsed = JSON.parse(block);
			return (Array.isArray(parsed) ? parsed : [parsed]).map((entry) => entry['@type']);
		});
		expect(types).toContain('WebApplication');
		expect(types).toContain('FAQPage');
	});

	test('robots.txt declares our sitemap', async ({ request }) => {
		const response = await request.get('/robots.txt');
		expect(response.status()).toBe(200);
		const body = await response.text();
		expect(body).toContain(`Sitemap: ${ORIGIN}/sitemap.xml`);
		expect(body).not.toContain('Content-Signal');
	});
});
