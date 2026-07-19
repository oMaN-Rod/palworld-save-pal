import { describe, expect, it } from 'vitest';
import { buildEditorTheme, rgbToHex, EDITOR_THEME_NAME } from './paletteTheme';

// A stand-in palette; only the vars the builder reads need entries. Distinct
// values per role so a mis-wired mapping is caught.
const palette: Record<string, string> = {
	'--color-primary-300': 'rgb(85, 159, 248)',
	'--color-primary-700': 'rgb(4, 83, 178)',
	'--color-secondary-300': 'rgb(159, 106, 218)',
	'--color-secondary-700': 'rgb(93, 34, 163)',
	'--color-tertiary-300': 'rgb(255, 82, 177)',
	'--color-tertiary-700': 'rgb(200, 0, 104)',
	'--color-success-300': 'rgb(105, 224, 162)',
	'--color-success-700': 'rgb(28, 156, 68)',
	'--color-surface-400': 'rgb(133, 133, 133)',
	'--color-surface-500': 'rgb(102, 102, 102)',
	'--color-surface-50': 'rgb(255, 255, 255)',
	'--color-surface-900': 'rgb(34, 34, 34)'
};
const read = (name: string) => palette[name] ?? '';

const ruleFor = (
	theme: ReturnType<typeof buildEditorTheme>,
	token: string
): string | undefined => theme.rules.find((r) => r.token === token)?.foreground;

describe('rgbToHex', () => {
	it('converts an rgb() string to 6-digit hex without a leading #', () => {
		expect(rgbToHex('rgb(1, 112, 243)')).toBe('0170f3');
	});

	it('zero-pads single-digit channels', () => {
		expect(rgbToHex('rgb(0, 8, 15)')).toBe('00080f');
	});

	it('tolerates rgba() and extra whitespace', () => {
		expect(rgbToHex('rgba( 34 , 34 ,34 , 0.5 )')).toBe('222222');
	});

	// Production CSS minification (Lightning CSS via Tailwind v4) rewrites the
	// theme's `rgb(...)` custom properties into these forms, which is what
	// getComputedStyle returns in a release build.
	it('parses 6-digit hex', () => {
		expect(rgbToHex('#559ff8')).toBe('559ff8');
	});

	it('parses hex with an alpha channel, dropping the alpha', () => {
		expect(rgbToHex('#559ff8ff')).toBe('559ff8');
	});

	it('expands shorthand hex', () => {
		expect(rgbToHex('#5af')).toBe('55aaff');
	});

	it('parses space-separated rgb()', () => {
		expect(rgbToHex('rgb(85 159 248)')).toBe('559ff8');
	});

	it('uppercases and leading/trailing whitespace are tolerated', () => {
		expect(rgbToHex('  #559FF8  ')).toBe('559ff8');
	});
});

describe('buildEditorTheme (dark)', () => {
	const theme = buildEditorTheme(read, false);

	it('uses the dark Monaco base', () => {
		expect(theme.base).toBe('vs-dark');
	});

	it('maps JSON tokens to their palette roles at the dark accent shade (300)', () => {
		expect(ruleFor(theme, 'string.key.json')).toBe('559ff8'); // primary-300
		expect(ruleFor(theme, 'string.value.json')).toBe('69e0a2'); // success-300
		expect(ruleFor(theme, 'number')).toBe('ff52b1'); // tertiary-300
		expect(ruleFor(theme, 'keyword.json')).toBe('9f6ada'); // secondary-300
	});

	it('paints editor chrome from the surface scale', () => {
		expect(theme.colors['editor.background']).toBe('#222222'); // surface-900
		expect(theme.colors['editor.foreground']).toBe('#ffffff'); // surface-50
	});
});

describe('buildEditorTheme (light)', () => {
	const theme = buildEditorTheme(read, true);

	it('uses the light Monaco base and the light accent shade (700)', () => {
		expect(theme.base).toBe('vs');
		expect(ruleFor(theme, 'string.key.json')).toBe('0453b2'); // primary-700
	});
});

describe('EDITOR_THEME_NAME', () => {
	it('is a stable registration key', () => {
		expect(EDITOR_THEME_NAME).toBe('psp-editor');
	});
});
