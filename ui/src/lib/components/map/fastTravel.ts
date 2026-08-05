/** Game class of watchtower fast travel points (vs `BP_LevelObject_TowerFastTravelPoint_C`). */
export const WATCHTOWER_CLASS = 'BP_LevelObject_UnlockMapPoint_C';

/** True when a fast travel point is a watchtower. Single source of truth for the class check. */
export function isWatchtower(point: { class?: string }): boolean {
	return point.class === WATCHTOWER_CLASS;
}
