import { persistedState } from 'svelte-persisted-state';
import type { SelectOption } from '$types';

export type ThemeName = 'dark' | 'frontier' | 'light';

export const DEFAULT_THEME: ThemeName = 'dark';

export const themeOptions: SelectOption[] = [
	{ value: 'dark', label: 'Dark' },
	{ value: 'frontier', label: 'Frontier' },
	{ value: 'light', label: 'Light' }
];

/**
 * Selected UI theme, persisted to localStorage. Changing `theme.current`
 * updates the persisted value; the `[data-theme]` attribute on <body> is kept
 * in sync from the root layout, which is what actually swaps the color palette.
 */
export const theme = persistedState<ThemeName>('psp-theme', DEFAULT_THEME);
