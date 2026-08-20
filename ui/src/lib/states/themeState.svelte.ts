import { persistedState } from 'svelte-persisted-state';
import type { SelectOption } from '$types';

export type ThemeName =
	| 'dark'
	| 'frontier'
	| 'light'
	| 'grizzbolt'
	| 'sakurajima'
	| 'wildlands'
	| 'ancient'
	| 'lamball';

export const DEFAULT_THEME: ThemeName = 'dark';

export const themeOptions: SelectOption[] = [
	{ value: 'dark', label: 'Dark' },
	{ value: 'frontier', label: 'Frontier' },
	{ value: 'light', label: 'Light' },
	{ value: 'grizzbolt', label: 'Grizzbolt' },
	{ value: 'sakurajima', label: 'Sakurajima' },
	{ value: 'wildlands', label: 'Wildlands' },
	{ value: 'ancient', label: 'Ancient Tech' },
	{ value: 'lamball', label: 'Lamball' }
];

// Persisted to localStorage; the `[data-theme]` attribute on <body> that actually
// swaps the color palette is kept in sync from the root layout, not from here.
export const theme = persistedState<ThemeName>('psp-theme', DEFAULT_THEME);

// Light-background themes (by --color-surface-950 in ui/src/themes/*.css); a new
// theme not added here silently gets the dark-logo variant.
export const LIGHT_THEMES: ReadonlySet<ThemeName> = new Set<ThemeName>(['light', 'lamball']);

export function isLightTheme(name: ThemeName): boolean {
	return LIGHT_THEMES.has(name);
}
