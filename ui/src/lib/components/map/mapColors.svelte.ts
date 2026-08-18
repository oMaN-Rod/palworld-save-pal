import { persistedState } from 'svelte-persisted-state';

export const DEFAULT_STRUCTURE_COLORS: Record<string, string> = {
	Foundation: '#9fa3a9',
	Furniture: '#e07a5f',
	Product: '#f4a261',
	Storage: '#8d99ae',
	Infrastructure: '#3d5a80',
	Pal: '#00b4d8',
	Light: '#f4d35e',
	Defense: '#e63946',
	Food: '#81b29a',
	Other: '#6c757d'
};

export const DEFAULT_MATERIAL_TINTS: Record<string, string> = {
	Wood: '#8b5a2b',
	Stone: '#9ba4b5',
	Metal: '#393e46',
	PalMetal: '#0077b6',
	Ancient: '#ffffff',
	Glass: '#90e0ef'
};

export const STRUCTURE_TYPE_ORDER = Object.keys(DEFAULT_STRUCTURE_COLORS);
export const MATERIAL_ORDER = Object.keys(DEFAULT_MATERIAL_TINTS);

export const DEFAULT_MATERIAL_OPACITY: Record<string, number> = Object.fromEntries(
	MATERIAL_ORDER.map((material) => [material, material === 'Glass' ? 0.4 : 1])
);

export const DEFAULT_MATERIAL_BLEND = 0.5;

export type MapColorProfile = {
	structures: Record<string, string>;
	materials: Record<string, string>;
	opacities: Record<string, number>;
	blend: number;
};

function stockProfile(): MapColorProfile {
	return {
		structures: { ...DEFAULT_STRUCTURE_COLORS },
		materials: { ...DEFAULT_MATERIAL_TINTS },
		opacities: { ...DEFAULT_MATERIAL_OPACITY },
		blend: DEFAULT_MATERIAL_BLEND
	};
}

export const mapColors = persistedState<MapColorProfile>('psp-map-colors', stockProfile());

const HEX_COLOR = /^#[0-9a-f]{6}$/i;

function mergeValidHex(
	defaults: Record<string, string>,
	stored: Record<string, string> | undefined
): Record<string, string> {
	const merged = { ...defaults };
	if (!stored) return merged;
	for (const [key, value] of Object.entries(stored)) {
		if (key in defaults && HEX_COLOR.test(value)) merged[key] = value;
	}
	return merged;
}

function mergeValidOpacity(
	defaults: Record<string, number>,
	stored: Record<string, number> | undefined
): Record<string, number> {
	const merged = { ...defaults };
	if (!stored) return merged;
	for (const [key, value] of Object.entries(stored)) {
		if (
			key in defaults &&
			typeof value === 'number' &&
			Number.isFinite(value) &&
			value >= 0 &&
			value <= 1
		) {
			merged[key] = value;
		}
	}
	return merged;
}

// structureFillColor calls these once per structure per layer rebuild -- half a
// million times while a large base loads -- and each derivation allocates and
// runs a regex per entry. Every setter replaces the stored slice rather than
// mutating it, so identity is a sound memo key. The slice is still read on every
// call, so a reader in an $effect keeps tracking mapColors; only the derivation
// is skipped. The result is frozen because it is now shared, not copied.
function memoizeByStored<S, T extends object>(derive: (stored: S) => T): (stored: S) => T {
	let lastStored: S;
	let lastValue: T;
	let primed = false;
	return (stored: S): T => {
		if (primed && lastStored === stored) return lastValue;
		lastValue = Object.freeze(derive(stored)) as T;
		lastStored = stored;
		primed = true;
		return lastValue;
	};
}

// persistedState hydrates verbatim, so a profile saved before a type existed
// resolves it as undefined, and a hand-edited value could be anything.
const deriveStructureColors = memoizeByStored((stored: Record<string, string> | undefined) =>
	mergeValidHex(DEFAULT_STRUCTURE_COLORS, stored)
);

const deriveMaterialTints = memoizeByStored((stored: Record<string, string> | undefined) =>
	mergeValidHex(DEFAULT_MATERIAL_TINTS, stored)
);

const deriveMaterialOpacities = memoizeByStored((stored: Record<string, number> | undefined) =>
	mergeValidOpacity(DEFAULT_MATERIAL_OPACITY, stored)
);

export function structureColors(): Record<string, string> {
	return deriveStructureColors(mapColors.current?.structures);
}

export function materialTints(): Record<string, string> {
	return deriveMaterialTints(mapColors.current?.materials);
}

export function materialOpacities(): Record<string, number> {
	return deriveMaterialOpacities(mapColors.current?.opacities);
}

export function materialOpacity(material?: string): number {
	if (!material || material === 'None') return 1;
	return materialOpacities()[material] ?? 1;
}

function clamp(value: number): number {
	return Math.min(1, Math.max(0, value));
}

export function materialBlend(): number {
	const value = mapColors.current?.blend;
	return typeof value === 'number' && Number.isFinite(value)
		? clamp(value)
		: DEFAULT_MATERIAL_BLEND;
}

export function setStructureColor(type: string, hex: string): void {
	if (structureColors()[type] === hex) return;
	mapColors.current = {
		...mapColors.current,
		structures: { ...mapColors.current.structures, [type]: hex }
	};
}

export function setMaterialTint(material: string, hex: string): void {
	if (materialTints()[material] === hex) return;
	mapColors.current = {
		...mapColors.current,
		materials: { ...mapColors.current.materials, [material]: hex }
	};
}

export function setMaterialOpacity(material: string, value: number): void {
	const next = clamp(value);
	if (materialOpacities()[material] === next) return;
	mapColors.current = {
		...mapColors.current,
		opacities: { ...mapColors.current?.opacities, [material]: next }
	};
}

export function setMaterialBlend(value: number): void {
	const next = clamp(value);
	if (materialBlend() === next) return;
	mapColors.current = { ...mapColors.current, blend: next };
}

export function resetMapColors(): void {
	mapColors.current = stockProfile();
}
