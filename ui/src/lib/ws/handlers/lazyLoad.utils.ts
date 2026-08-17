export type PlayerDetailsOrigin = 'edit' | 'bulk' | 'map' | 'reattach' | 'breeding';

export interface PlayerDetailsRouting {
	/** Where the loaded player should be stored in app state. */
	target: 'selected' | 'bulkDetail';
	/** Route to navigate to after loading, or null to stay on the current page. */
	navigateTo: string | null;
}

/**
 * Decides how a lazily-loaded player detail response should be applied, based on
 * the `origin` that requested it. Only the `edit` origin navigates away; callers
 * that just need the player in state (e.g. the world map or the breeding
 * calculator) pass their own origin to stay put.
 */
export function resolvePlayerDetailsRouting(origin: string | undefined): PlayerDetailsRouting {
	if (origin === 'bulk') {
		return { target: 'bulkDetail', navigateTo: null };
	}
	if (origin === 'map' || origin === 'reattach' || origin === 'breeding') {
		// Load into state, stay on whatever route reloaded / requested it. The
		// breeding calculator requests player details to read pals but must not
		// be navigated away to the editor.
		return { target: 'selected', navigateTo: null };
	}
	// Default (edit) path: select the player and open the editor.
	return { target: 'selected', navigateTo: '/edit/player' };
}
