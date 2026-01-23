import { goto } from '$app/navigation';
import { palsData } from '$lib/data';
import { getAppState } from '$states';
import type { Guild, GuildSummary, Player, PlayerSummary } from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

export const getPlayerSummariesHandler: WSMessageHandler = {
	type: MessageType.GET_PLAYER_SUMMARIES,
	async handle(data: Record<string, PlayerSummary>) {
		const appState = getAppState();
		console.log('Received player summaries', Object.keys(data).length);
		appState.playerSummaries = data;
		goto('/edit');
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
	async handle(data: { player: Player; player_id: string } | { error: string }) {
		const appState = getAppState();

		if ('error' in data) {
			console.error('Failed to load player details:', data.error);
			appState.loadingPlayer = false;
			return;
		}

		const { player, player_id } = data;
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

		// Add to players cache
		appState.players[player_id] = player;

		// Update summary to show as loaded
		if (appState.playerSummaries[player_id]) {
			appState.playerSummaries[player_id].loaded = true;
		}

		// Set as selected player
		appState.selectedPlayer = player;
		appState.selectedPlayerUid = player_id;
		appState.loadingPlayer = false;
		goto('/edit/player');
	}
};

export const getGuildDetailsResponseHandler: WSMessageHandler = {
	type: MessageType.GET_GUILD_DETAILS_RESPONSE,
	async handle(data: { guild: Guild; guild_id: string } | { error: string }) {
		const appState = getAppState();

		if ('error' in data) {
			console.error('Failed to load guild details:', data.error);
			appState.loadingGuild = false;
			return;
		}

		const { guild, guild_id } = data;
		console.log('Received guild details for', guild.name);

		// Add to guilds cache
		appState.guilds[guild_id] = guild;

		// Update summary to show as loaded
		if (appState.guildSummaries[guild_id]) {
			appState.guildSummaries[guild_id].loaded = true;
		}

		appState.loadingGuild = false;
	}
};

export const lazyLoadHandlers = [
	getPlayerSummariesHandler,
	getGuildSummariesHandler,
	getPlayerDetailsResponseHandler,
	getGuildDetailsResponseHandler
];
