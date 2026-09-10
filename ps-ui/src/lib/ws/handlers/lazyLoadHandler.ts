import { goto } from '$app/navigation';
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

		if (player.pals) {
			Object.values(player.pals).forEach((pal) => {
				const palInfo = palsData.getByKey(pal.character_key);
				if (!palInfo) {
					console.error(`Failed to find pal info for`, JSON.parse(JSON.stringify(pal)));
				}
				pal.name = palInfo?.localized_name || pal.character_id;
			});
		}

		// Read the stored (proxied) value back rather than reusing `player` directly,
		// so selectedPlayer/bulkDetailPlayer are the SAME reactive proxy as
		// players[player_id] -- a raw assignment would yield a separate proxy, and
		// edits to it would never reach the players[] entry saveState iterates.
		appState.players[player_id] = player;
		const stored = appState.players[player_id];

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

		// Reference the stored (proxied) value so bulkDetailGuild is the SAME reactive
		// proxy as guilds[guild_id] (see the player handler above).
		appState.guilds[guild_id] = guild;
		const storedGuild = appState.guilds[guild_id];

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
