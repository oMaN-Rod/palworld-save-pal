export type WikiViewMode = 'list' | 'grid';

/**
 * Shared wiki browsing preferences. Module-level `$state` so the list/grid
 * choice persists across tab navigation within a session.
 */
export const wikiPrefs = $state({ viewMode: 'list' as WikiViewMode });
