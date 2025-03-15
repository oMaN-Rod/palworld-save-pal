import { palsData } from '$lib/data';
import { getAppState, getToastState } from '$states';
import type { Player } from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

const appState = getAppState();
const toast = getToastState();

export const getPlayersHandler: WSMessageHandler = {
	type: MessageType.GET_PLAYERS,
	async handle(data: Record<string, Player>) {
		const processedPlayers = await Promise.all(
			Object.entries(data).map(async ([key, player]) => {
				try {
					if (player.pals) {
						Object.values(player.pals).map((pal) => {
							const palInfo = palsData.pals[pal.character_key];
							if (!palInfo) {
								console.error(`Failed to find pal info for`, JSON.parse(JSON.stringify(pal)));
							}
							pal.name = palInfo?.localized_name || pal.character_id;
						});
					}
					return [key, player];
				} catch (error) {
					console.error(`Failed to parse player data for key ${key}:`, error);
					return null;
				}
			})
		);

		appState.players = Object.fromEntries(
			processedPlayers.filter((entry): entry is [string, Player] => entry !== null)
		);
	}
};

export const deletePlayerHandler: WSMessageHandler = {
	type: MessageType.DELETE_PLAYER,
	async handle(data: Record<string, any>, { goto }) {
		const { player_id, origin } = data;
		if (!data) {
			toast.add(
				'Cannot delete guild admin player, you must delete the entire guild or transfer admin to another player',
				'Error',
				'error'
			);
			return;
		}
		appState.selectedPlayer = undefined;
		appState.selectedPal = undefined;
		const nickname = appState.players[player_id]?.nickname || 'Unknown Player';
		delete appState.players[player_id];
		Object.values(appState.guilds).forEach((guild) => {
			guild.players = guild.players.filter((player) => player !== player_id);
		});
		toast.add(`Player ${nickname} deleted`, undefined, 'success');
		goto(`/${origin}`);
	}
};

export const playerHandlers = [deletePlayerHandler, getPlayersHandler];
