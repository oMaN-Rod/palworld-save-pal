import type * as MonacoE from 'monaco-editor';

/** Registration key for the palette-derived editor theme. */
export const EDITOR_THEME_NAME = 'psp-editor';

/** Reads a CSS custom property's value, e.g. `--color-primary-300` → `rgb(...)`. */
export type VarReader = (name: string) => string;

/** A CSS color → `0170f3` (bare 6-digit hex; editor `colors` want a leading `#`,
 *  added by the caller). Handles the forms getComputedStyle actually returns:
 *  `rgb(1, 112, 243)` and `rgba(...)` in dev, but hex (`#0170f3`) or
 *  space-separated `rgb(1 112 243)` once production CSS minification (Lightning
 *  CSS via Tailwind v4) rewrites the theme's `rgb(...)` custom properties. */
export function rgbToHex(color: string): string {
	const value = color.trim();

	const hexMatch = value.match(/^#([0-9a-fA-F]{3,8})$/);
	if (hexMatch) {
		let digits = hexMatch[1];
		if (digits.length === 3 || digits.length === 4) {
			digits = digits
				.slice(0, 3)
				.split('')
				.map((c) => c + c)
				.join('');
		}
		return digits.slice(0, 6).padEnd(6, '0').toLowerCase();
	}

	const channels = value.match(/\d+(?:\.\d+)?/g);
	if (!channels || channels.length < 3) return '000000';
	return channels
		.slice(0, 3)
		.map((channel) => Math.round(Number(channel)).toString(16).padStart(2, '0'))
		.join('');
}

/**
 * A Monaco theme whose JSON syntax colors come from the live app palette, so the
 * editor matches the active app theme. Roles: keys→primary, string values→
 * success, numbers→tertiary, keywords (true/false/null)→secondary, punctuation→
 * muted surface. Accent shades lighten on dark backgrounds and darken on light
 * ones for legible contrast against the surface background.
 *
 * `readVar` is injected so this stays pure and DOM-free (and unit-testable); the
 * editor passes a reader backed by `getComputedStyle` on a probe element.
 */
export function buildEditorTheme(
	readVar: VarReader,
	isLight: boolean
): MonacoE.editor.IStandaloneThemeData {
	const accent = isLight ? '700' : '300';
	const muted = isLight ? '500' : '400';
	const hex = (name: string) => rgbToHex(readVar(name));

	const primary = hex(`--color-primary-${accent}`);
	const secondary = hex(`--color-secondary-${accent}`);
	const tertiary = hex(`--color-tertiary-${accent}`);
	const success = hex(`--color-success-${accent}`);
	const surfaceMuted = hex(`--color-surface-${muted}`);
	const background = hex('--color-surface-900');
	const foreground = hex('--color-surface-50');
	const lineNumber = hex('--color-surface-500');

	return {
		base: isLight ? 'vs' : 'vs-dark',
		inherit: true,
		rules: [
			{ token: 'string.key.json', foreground: primary },
			{ token: 'string.value.json', foreground: success },
			{ token: 'string', foreground: success },
			{ token: 'number', foreground: tertiary },
			{ token: 'keyword.json', foreground: secondary },
			{ token: 'keyword', foreground: secondary },
			{ token: 'delimiter', foreground: surfaceMuted }
		],
		colors: {
			'editor.background': `#${background}`,
			'editor.foreground': `#${foreground}`,
			'editorLineNumber.foreground': `#${lineNumber}`,
			'editorLineNumber.activeForeground': `#${primary}`,
			'editorCursor.foreground': `#${primary}`,
			'editor.lineHighlightBackground': `#${primary}1a`,
			'editor.selectionBackground': `#${primary}44`
		}
	};
}
