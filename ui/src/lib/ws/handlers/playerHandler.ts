import { palsData } from '$lib/data';
import { getAppState } from '$states';
import type { Player } from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

const appState = getAppState();

export const getPlayersHandler: WSMessageHandler = {
	type: MessageType.GET_PLAYERS,
	async handle(data: Record<string, Player>, { goto }) {
		console.log('Players loaded', data);

		const processedPlayers = await Promise.all(
			Object.entries(data).map(async ([key, player]) => {
				try {
					if (player.pals) {
						await Promise.all(
							Object.values(player.pals).map(async (pal) => {
								const palInfo = await palsData.getPalInfo(pal.character_id);
								if (!palInfo) {
									console.error(`Failed to find pal info for`, JSON.parse(JSON.stringify(pal)));
								}
								pal.name = palInfo?.localized_name || pal.character_id;
							})
						);
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

		await goto('/edit');
	}
};

export const playerHandlers = [getPlayersHandler];
