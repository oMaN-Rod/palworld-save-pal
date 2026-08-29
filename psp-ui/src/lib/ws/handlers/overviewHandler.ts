import type { OverviewStats } from '$states';
import { getOverviewState, getToastState } from '$states';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';
import { browserDownload, handleExportFrame } from './blueprintHandler';
import * as m from '$i18n/messages';

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

// Desktop answers with {message, file_path} — the backend already wrote the
// file, because the webview ignores <a download>. The browser gets
// [{name, content}] to download instead.
export const exportOverviewStatsHandler: WSMessageHandler = {
	type: MessageType.EXPORT_OVERVIEW_STATS,
	async handle(data) {
		const toast = getToastState();
		if (data && typeof data === 'object' && 'error' in data) {
			toast.add(String(data.error), m.overview_export_json(), 'error');
			return;
		}
		handleExportFrame(data, {
			download: browserDownload,
			toast: (message) => toast.add(message, m.overview_export_json(), 'success')
		});
	}
};

export const overviewHandlers = [getOverviewStatsHandler, exportOverviewStatsHandler];
