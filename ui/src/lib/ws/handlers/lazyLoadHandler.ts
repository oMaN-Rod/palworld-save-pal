import { goto } from '$app/navigation';
import { page } from '$app/state';
import { palsData } from '$lib/data';
import { getAppState } from '$states';
import type { Guild, GuildSummary, Player, PlayerSummary } from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';
import { resolvePlayerDetailsRouting } from './lazyLoad.utils';

export const getPlayerSummariesHandler: WSMessageHandler = {
	type: MessageType.GET_PLAYER_SUMMARIES,
	async handle(data: Record<string, PlayerSummary>) {
		const appState = getAppState();
		console.log('Received player summaries', Object.keys(data).length);
		appState.playerSummaries = data;
		// Only navigate to /edit if not already on /bulk page
		if (!page.url.pathname.startsWith('/bulk')) {
			goto('/edit');
		}
	}
};

export const getGuildSummariesHandler: WSMessageHandler = {
	type: MessageType.GET_GUILD_SUMMARIES,
	async handle(data: Record<string, GuildSummary>) {
		const appState = getAppState();
		console.log('Received guild summaries', Object.keys(data).length);
		appState.guildSummaries = data;
	}
};

export const getPlayerDetailsResponseHandler: WSMessageHandler = {
	type: MessageType.GET_PLAYER_DETAILS_RESPONSE,
	async handle(data: { player: Player; player_id: string; origin?: string } | { error: string }) {
		const appState = getAppState();

		if ('error' in data) {
			console.error('Failed to load player details:', data.error);
			appState.loadingPlayer = false;
			return;
		}

		const { player, player_id, origin } = data;
		console.log('Received player details for', player.nickname);

		// Process pals to add localized names
		if (player.pals) {
			Object.values(player.pals).forEach((pal) => {
				const palInfo = palsData.getByKey(pal.character_key);
				if (!palInfo) {
					console.error(`Failed to find pal info for`, JSON.parse(JSON.stringify(pal)));
				}
				pal.name = palInfo?.localized_name || pal.character_id;
			});
		}

		// Add to players cache. Read the stored (proxied) value back for
		// selectedPlayer/bulkDetailPlayer so they are the SAME reactive proxy
		// as players[player_id]; assigning the raw `player` yields a separate
		// proxy, so edits that set `selectedPlayer.state` never reach the
		// players[] entry saveState iterates.
		appState.players[player_id] = player;
		const stored = appState.players[player_id];

		// Update summary to show as loaded
		if (appState.playerSummaries[player_id]) {
			appState.playerSummaries[player_id].loaded = true;
		}

		appState.loadingPlayer = false;

		const routing = resolvePlayerDetailsRouting(origin);
		if (routing.target === 'bulkDetail') {
			appState.bulkDetailPlayer = stored;
			return;
		}

		appState.selectedPlayer = stored;
		appState.selectedPlayerUid = player_id;
		if (routing.navigateTo) goto(routing.navigateTo);
	}
};

export const getGuildDetailsResponseHandler: WSMessageHandler = {
	type: MessageType.GET_GUILD_DETAILS_RESPONSE,
	async handle(data: { guild: Guild; guild_id: string } | { error: string }) {
		const appState = getAppState();

		if ('error' in data) {
			console.error('Failed to load guild details:', data.error);
			appState.loadingGuild = false;
			appState.bulkGuildRequestPending = false;
			return;
		}

		const { guild, guild_id } = data;
		console.log('Received guild details for', guild.name);

		// Add to guilds cache, then reference the stored (proxied) value so
		// bulkDetailGuild is the SAME reactive proxy as guilds[guild_id] (a raw
		// assignment yields a separate proxy — see the player handler above).
		appState.guilds[guild_id] = guild;
		const storedGuild = appState.guilds[guild_id];

		// Update summary to show as loaded
		if (appState.guildSummaries[guild_id]) {
			appState.guildSummaries[guild_id].loaded = true;
		}

		appState.loadingGuild = false;
		if (appState.bulkGuildRequestPending) {
			appState.bulkDetailGuild = storedGuild;
			appState.bulkGuildRequestPending = false;
		}
	}
};

export const lazyLoadHandlers = [
	getPlayerSummariesHandler,
	getGuildSummariesHandler,
	getPlayerDetailsResponseHandler,
	getGuildDetailsResponseHandler
];
