import { palsData } from '$lib/data';
import { getAppState, getNavigationState } from '$states';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

export const addPalHandler: WSMessageHandler = {
	type: MessageType.ADD_PAL,
	async handle(data) {
		const { player_id, guild_id, base_id, pal } = data;
		const appState = getAppState();
		const nav = getNavigationState();

		if (!pal) {
			return;
		}

		if (player_id && appState.players) {
			const palData = palsData.getPalData(pal.character_key);
			pal.name = palData?.localized_name || pal.character_id;
			pal.elements = palData?.element_types || [];
			appState.players[player_id].pals![pal.instance_id] = pal;
		}

		if (guild_id && appState.guilds) {
			const palData = palsData.getPalData(pal.character_key);
			pal.name = palData?.localized_name || pal.character_id;
			pal.elements = palData?.element_types || [];
			appState.guilds[guild_id].bases[base_id].pals[pal.instance_id] = pal;
		}

		appState.selectedPal = pal;
		nav.activeTab = 'pal';
	}
};

export const addDpsPalHandler: WSMessageHandler = {
	type: MessageType.ADD_DPS_PAL,
	async handle(data) {
		const { player_id, pal, index } = data;
		const appState = getAppState();
		const nav = getNavigationState();

		if (!pal) {
			return;
		}

		if (player_id && appState.players) {
			const palData = palsData.getPalData(pal.character_key);
			pal.name = palData?.localized_name || pal.character_id;
			pal.elements = palData?.element_types || [];
			if (appState.players[player_id].dps) {
				appState.players[player_id].dps[index] = pal;
			}
		}

		appState.selectedPal = pal;
		nav.activeTab = 'pal';
	}
};

export const movePalHandler: WSMessageHandler = {
	type: MessageType.MOVE_PAL,
	async handle(data) {
		const appState = getAppState();

		const move_data = data as {
			player_id: string;
			pal_id: string;
			container_id: string;
			slot_index: number;
		};
		if (appState.players && appState.players[move_data.player_id]) {
			const player = appState.players[move_data.player_id];
			const pal = player.pals ? player.pals[move_data.pal_id] : undefined;
			if (pal) {
				pal.storage_id = move_data.container_id;
				pal.storage_slot = move_data.slot_index;
			}
		}
	}
};

export const palHandlers = [addPalHandler, movePalHandler, addDpsPalHandler];
