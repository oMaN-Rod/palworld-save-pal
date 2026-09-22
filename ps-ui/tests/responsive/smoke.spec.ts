import { expect, test } from '@playwright/test';

import { CORE_ROUTES } from './routes';

const MIN_TOUCH_PX = 44;

type Offender = { label: string; width: number; height: number };

for (const route of CORE_ROUTES) {
	test(`${route} renders its main landmark`, async ({ page }) => {
		await page.goto(route);
		await page.waitForLoadState('networkidle');

		await expect(page.locator('main')).toHaveCount(1);
	});

	test(`${route} gives every control a thumb-sized target`, async ({ page }, testInfo) => {
		test.skip(!testInfo.project.use.hasTouch, 'pointer is not a finger here');

		await page.goto(route);
		await page.waitForLoadState('networkidle');

		const offenders: Offender[] = await page.evaluate((min) => {
			const label = (el: Element) =>
				`${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ''}: ${
					el.getAttribute('aria-label') ?? el.textContent?.trim().slice(0, 30) ?? ''
				}`;

			return [...document.querySelectorAll('button, a[href]')]
				.filter((el) => {
					const style = getComputedStyle(el);
					if (style.visibility === 'hidden' || style.display === 'none') return false;
					if ((el as HTMLButtonElement).disabled) return false;
					const box = el.getBoundingClientRect();
					if (box.width === 0 || box.height === 0) return false;
					return box.width < min || box.height < min;
				})
				.map((el) => {
					const box = el.getBoundingClientRect();
					return {
						label: label(el),
						width: Math.round(box.width),
						height: Math.round(box.height)
					};
				});
		}, MIN_TOUCH_PX);

		expect(
			offenders,
			`controls under ${MIN_TOUCH_PX}px on ${route}:\n${offenders
				.map((o) => `  ${o.width}x${o.height} ${o.label}`)
				.join('\n')}`
		).toEqual([]);
	});
}
