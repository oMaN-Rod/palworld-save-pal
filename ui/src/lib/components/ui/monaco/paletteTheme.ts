import type * as MonacoE from 'monaco-editor';

/** Registration key for the palette-derived editor theme. */
export const EDITOR_THEME_NAME = 'psp-editor';

/** Reads a CSS custom property's value, e.g. `--color-primary-300` → `rgb(...)`. */
export type VarReader = (name: string) => string;

/** `rgb(1, 112, 243)` (or `rgba(...)`) → `0170f3`. Monaco token foregrounds are
 *  bare 6-digit hex; editor `colors` want a leading `#`, added by the caller. */
export function rgbToHex(rgb: string): string {
	const match = rgb.match(/(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/);
	if (!match) return '000000';
	return match
		.slice(1, 4)
		.map((channel) => Number(channel).toString(16).padStart(2, '0'))
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
