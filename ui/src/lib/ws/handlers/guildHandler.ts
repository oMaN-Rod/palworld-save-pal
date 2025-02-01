import { palsData } from '$lib/data';
import { getAppState } from '$states';
import type { Guild } from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

const appState = getAppState();

export const getGuildsHandler: WSMessageHandler = {
	type: MessageType.GET_GUILDS,
	async handle(data: Record<string, Guild>) {
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
	}
};

export const guildHandlers = [getGuildsHandler];
