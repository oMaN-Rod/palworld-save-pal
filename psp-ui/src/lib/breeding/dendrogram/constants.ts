export const DENDRO_CONFIG = {
	nodeWidth: 168,
	nodeHeight: 52,
	levelGap: 72,
	siblingGap: 14,
	iconSize: 38,
	iconPadding: 7,
	zoom: { min: 0.2, max: 3, factor: 1.2 },
	animation: { durationMs: 400 },
	fitMargin: 32,
	targetNodeWidth: 188
} as const;

export const DENDRO_COLORS = {
	bgCard: '#161E2D',
	bgCardSelected: '#1A2332',
	bgCardHover: '#1E2633',
	bgCardBred: '#131A28',
	bgDeep: '#0A1018',
	line: '#2A3A4A',
	lineActive: '#3A4A5A',
	accent: '#3B8ED0',
	accentLight: '#5BA3E0',
	accentCyan: '#00BCD4',
	accentTarget: '#4FC3F7',
	inkPrimary: '#E3F2FD',
	inkSecondary: '#B0BEC5',
	inkDim: '#546E7A',
	owned: '#3B8ED0',
	selected: '#4CAF50',
	wild: '#FFB74D',
	male: '#5BA3E0',
	female: '#F48FB1',
	wildcard: '#546E7A',
	passiveMatched: '#4CAF50',
	passiveOther: '#3A4A5C',
	link: '#3A6A9A',
	linkActive: '#5BAEE0',
	linkHighlight: '#7BC4F0'
} as const;

export type DendroColors = { -readonly [K in keyof typeof DENDRO_COLORS]: string };

const COLOR_TO_VAR: Record<keyof typeof DENDRO_COLORS, string> = {
	bgCard: '--color-surface-900',
	bgCardSelected: '--color-surface-800',
	bgCardHover: '--color-surface-700',
	bgCardBred: '--color-surface-950',
	bgDeep: '--color-surface-950',
	line: '--color-surface-600',
	lineActive: '--color-surface-500',
	accent: '--color-primary-400',
	accentLight: '--color-primary-300',
	accentCyan: '--color-primary-400',
	accentTarget: '--color-primary-300',
	inkPrimary: '--color-surface-50',
	inkSecondary: '--color-surface-400',
	inkDim: '--color-surface-500',
	owned: '--color-primary-400',
	selected: '--color-success-400',
	wild: '--color-warning-400',
	male: '--color-primary-300',
	female: '--color-tertiary-400',
	wildcard: '--color-surface-500',
	passiveMatched: '--color-success-400',
	passiveOther: '--color-surface-500',
	link: '--color-primary-600',
	linkActive: '--color-primary-400',
	linkHighlight: '--color-primary-300'
};

export function resolveDendroColors(): DendroColors {
	const out: DendroColors = { ...DENDRO_COLORS };
	if (typeof document === 'undefined' || typeof getComputedStyle === 'undefined') {
		return out;
	}
	const style = getComputedStyle(document.body);
	for (const key of Object.keys(COLOR_TO_VAR) as (keyof typeof DENDRO_COLORS)[]) {
		const value = style.getPropertyValue(COLOR_TO_VAR[key]).trim();
		if (value) out[key] = value;
	}
	return out;
}
