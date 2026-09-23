import { expect, test } from '@playwright/test';

import { discoverRoutes } from './routes';

const MIN = 44;

test.beforeEach(({ hasTouch }) => {
	test.skip(!hasTouch, 'the minimum only applies to a touch pointer');
});

for (const route of discoverRoutes()) {
	test(`${route} controls are thumb-sized`, async ({ page }) => {
		await page.goto(route);
		await page.waitForLoadState('networkidle');

		const small = await page.evaluate((min) => {
			const out: string[] = [];
			const seen = new Set<Element>();

			for (const el of document.querySelectorAll('a, button, input, select, [role="tab"]')) {
				// Third-party editor chrome.
				if (el.closest('.jse-main, .monaco-editor')) continue;
				// Inline links fall under the standard's inline exception.
				if (el.tagName === 'A' && el.closest('.prose-ps')) continue;
				if (el instanceof HTMLInputElement && el.type === 'file') continue;

				// A labelled field is aimed at through its label, e.g. a switch's 1px input.
				const target = el.closest('label') ?? el;
				if (seen.has(target)) continue;
				seen.add(target);

				const rect = target.getBoundingClientRect();
				if (rect.width === 0 || rect.height === 0) continue;
				if (rect.width >= min && rect.height >= min) continue;

				const label = (el.getAttribute('aria-label') ?? target.textContent ?? '')
					.trim()
					.slice(0, 30);
				out.push(`${Math.round(rect.width)}x${Math.round(rect.height)} "${label}"`);
			}
			return out;
		}, MIN);

		expect(small, `${route} has controls under ${MIN}px:\n${small.join('\n')}`).toEqual([]);
	});
}
