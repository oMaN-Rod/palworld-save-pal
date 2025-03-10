import { palsData } from '$lib/data';
import { getAppState, getToastState } from '$states';
import type { Guild } from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

const appState = getAppState();
const toast = getToastState();

export const getGuildsHandler: WSMessageHandler = {
	type: MessageType.GET_GUILDS,
	async handle(data: Record<string, Guild>, { goto }) {
		const processedGuilds = await Promise.all(
			Object.entries(data).map(async ([key, guild]) => {
				try {
					if (guild.bases) {
						await Promise.all(
							Object.values(guild.bases).map(async (base) => {
								if (base.pals) {
									await Promise.all(
										Object.values(base.pals).map(async (pal) => {
											const palInfo = palsData.pals[pal.character_key];
											if (!palInfo) {
												console.error(
													`Failed to find pal info for`,
													JSON.parse(JSON.stringify(pal))
												);
											}
											pal.name = palInfo?.localized_name || pal.character_id;
										})
									);
								}
							})
						);
					}
					return [key, guild];
				} catch (error) {
					console.error(`Failed to parse guild data for key ${key}:`, error);
					return null;
				}
			})
		);
		appState.guilds = Object.fromEntries(
			processedGuilds.filter((entry): entry is [string, Guild] => entry !== null)
		);
		console.log(`Loaded ${Object.keys(appState.guilds).length} guilds`);

		await goto('/edit');
	}
};

export const deleteGuildHandler: WSMessageHandler = {
	type: MessageType.DELETE_GUILD,
	async handle(data: Record<string, any>, { goto }) {
		console.log(`Deleting guild ${JSON.stringify(data)}`);
		const { guild_id, origin } = data;
		appState.selectedPlayer = undefined;
		appState.selectedPal = undefined;
		const guild = appState.guilds[guild_id];
		const guildName = guild?.name || 'Unknown Guild';
		appState.players = Object.fromEntries(
			Object.entries(appState.players).filter(([key]) => guild.players?.includes(key) !== true)
		);
		delete appState.guilds[guild_id];
		toast.add(`Guild ${guildName} deleted`, undefined, 'success');
		await goto(`/${origin}`);
	}
};

export const guildHandlers = [deleteGuildHandler, getGuildsHandler];
