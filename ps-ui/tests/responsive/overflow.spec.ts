import { expect, test } from '@playwright/test';

import { discoverRoutes } from './routes';

for (const route of discoverRoutes()) {
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

	// `overflow-hidden` shells hide a too-wide child from the check above.
	test(`${route} keeps its content on the screen`, async ({ page }) => {
		await page.goto(route);
		await page.waitForLoadState('networkidle');

		const escaped = await page.evaluate(() => {
			const out: string[] = [];
			const limit = document.documentElement.clientWidth;
			for (const el of document.querySelectorAll('body *')) {
				const rect = el.getBoundingClientRect();
				if (rect.width === 0 || rect.height === 0) continue;
				if (rect.right <= limit + 1 && rect.left >= -1) continue;
				if (getComputedStyle(el).position === 'fixed') continue;

				let scrolls = false;
				for (let parent = el.parentElement; parent; parent = parent.parentElement) {
					const overflowX = getComputedStyle(parent).overflowX;
					if (overflowX === 'auto' || overflowX === 'scroll') {
						scrolls = true;
						break;
					}
				}
				if (scrolls) continue;

				const name = (el.id || el.className || '').toString().slice(0, 45);
				out.push(
					`<${el.tagName.toLowerCase()} ${name}> ${Math.round(rect.left)}..${Math.round(rect.right)} of ${limit}`
				);
			}
			return out;
		});

		expect(escaped, `${route} cuts content off:\n${escaped.join('\n')}`).toEqual([]);
	});
}
