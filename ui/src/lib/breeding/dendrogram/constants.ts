/**
 * Dendrogram layout config + colors.
 *
 * Colors are hardcoded hex values tuned for PSP's dark theme. They match the
 * breeding tab's surrounding HTML UI. If the theme changes, update here too —
 * there is no build-time link between Tailwind and inline SVG attributes.
 */
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
