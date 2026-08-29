export type PlayerDetailsOrigin = 'edit' | 'bulk' | 'map' | 'reattach' | 'breeding';

export interface PlayerDetailsRouting {
	target: 'selected' | 'bulkDetail';
	navigateTo: string | null;
}

export function resolvePlayerDetailsRouting(origin: string | undefined): PlayerDetailsRouting {
	if (origin === 'bulk') {
		return { target: 'bulkDetail', navigateTo: null };
	}
	if (origin === 'map' || origin === 'reattach' || origin === 'breeding') {
		return { target: 'selected', navigateTo: null };
	}
	return { target: 'selected', navigateTo: '/edit/player' };
}
