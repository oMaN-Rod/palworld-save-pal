export type SheetSnap = 'peek' | 'half' | 'tall';

export const SHEET_SNAP_VH: Record<SheetSnap, number> = {
	peek: 38,
	half: 60,
	tall: 85
};

const DRAG_THRESHOLD_PX = 48;

export const DRAG_MOVE_EPSILON_PX = 4;

export const DEFAULT_SNAPS: SheetSnap[] = ['peek', 'tall'];

export function resolveDrag(
	current: SheetSnap,
	deltaY: number,
	snaps: SheetSnap[] = DEFAULT_SNAPS,
	threshold = DRAG_THRESHOLD_PX
): SheetSnap | 'closed' {
	const index = snaps.indexOf(current);
	if (index === -1) return current;

	if (deltaY > threshold) {
		return index === 0 ? 'closed' : snaps[index - 1];
	}
	if (deltaY < -threshold) {
		return index === snaps.length - 1 ? current : snaps[index + 1];
	}
	return current;
}
