export type SheetSnap = 'peek' | 'tall';

export const SHEET_SNAP_VH: Record<SheetSnap, number> = {
	peek: 38,
	tall: 85
};

const DRAG_THRESHOLD_PX = 48;

// Downward past the threshold steps one snap toward dismissal; upward steps one
// toward `tall` and stops there.
export function resolveDrag(
	current: SheetSnap,
	deltaY: number,
	threshold = DRAG_THRESHOLD_PX
): SheetSnap | 'closed' {
	if (deltaY > threshold) return current === 'tall' ? 'peek' : 'closed';
	if (deltaY < -threshold) return 'tall';
	return current;
}
