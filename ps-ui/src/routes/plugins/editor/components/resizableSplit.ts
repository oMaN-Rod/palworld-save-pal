export type SplitOrientation = 'horizontal' | 'vertical';

export const SPLIT_MIN_RATIO = 0.15;
export const SPLIT_MAX_RATIO = 0.85;
export const DEFAULT_SPLIT_RATIO = 0.75;

export function clampSplitRatio(ratio: number): number {
	if (!Number.isFinite(ratio)) return DEFAULT_SPLIT_RATIO;
	return Math.min(SPLIT_MAX_RATIO, Math.max(SPLIT_MIN_RATIO, ratio));
}

export function ratioFromPointer(pointer: number, start: number, size: number): number {
	if (!(size > 0)) return DEFAULT_SPLIT_RATIO;
	return clampSplitRatio((pointer - start) / size);
}

export function toggleOrientation(orientation: SplitOrientation): SplitOrientation {
	return orientation === 'horizontal' ? 'vertical' : 'horizontal';
}
