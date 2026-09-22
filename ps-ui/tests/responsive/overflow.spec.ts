import { expect, test } from '@playwright/test';

import { CORE_ROUTES } from './routes';

// The assertion jsdom cannot make: nothing in the page may be wider than the
// page. A single un-wrapped grid or a fixed width is enough to break it, and on
// a phone it shows up as a page that slides sideways under the thumb.
for (const route of CORE_ROUTES) {
	test(`${route} does not scroll sideways`, async ({ page }) => {
		await page.goto(route);
		await page.waitForLoadState('networkidle');

		const { scrollWidth, clientWidth } = await page.evaluate(() => {
			const el = document.scrollingElement as HTMLElement;
			return { scrollWidth: el.scrollWidth, clientWidth: el.clientWidth };
		});

		expect(scrollWidth, `${route} overflows by ${scrollWidth - clientWidth}px`).toBeLessThanOrEqual(
			clientWidth
		);
	});
}
