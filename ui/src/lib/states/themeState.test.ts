import { describe, it, expect } from 'vitest';
import { isLightTheme } from './themeState.svelte';

describe('isLightTheme', () => {
	it('classifies light-background themes', () => {
		expect(isLightTheme('light')).toBe(true);
		expect(isLightTheme('lamball')).toBe(true);
	});
	it('classifies dark themes', () => {
		for (const t of ['dark', 'frontier', 'grizzbolt', 'sakurajima', 'wildlands', 'ancient'] as const) {
			expect(isLightTheme(t)).toBe(false);
		}
	});
});
