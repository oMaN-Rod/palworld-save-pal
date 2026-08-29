import { getAppState } from '$states';
import type { PageLoad } from './$types';

export const load: PageLoad = async () => {
	const appState = getAppState();

	if (appState.selectedPlayer?.guild_id) {
		const guildId = appState.selectedPlayer.guild_id;

		if (!appState.guilds[guildId]) {
			await appState.loadGuildLazy(guildId);
		}
	}

	return {
		guildLoading: appState.loadingGuild
	};
};
