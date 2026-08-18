import type { OverviewStats } from '$states';
import { getOverviewState } from '$states';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

export const getOverviewStatsHandler: WSMessageHandler = {
	type: MessageType.GET_OVERVIEW_STATS,
	async handle(data: { stats: OverviewStats } | { error: string }) {
		const overviewState = getOverviewState();
		if ('error' in data) {
			console.error('Failed to load overview stats:', data.error);
			overviewState.setError(data.error);
			return;
		}
		overviewState.setStats(data.stats);
	}
};

export const overviewHandlers = [getOverviewStatsHandler];
