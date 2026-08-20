export const WATCHTOWER_CLASS = 'BP_LevelObject_UnlockMapPoint_C';

export function isWatchtower(point: { class?: string }): boolean {
	return point.class === WATCHTOWER_CLASS;
}
