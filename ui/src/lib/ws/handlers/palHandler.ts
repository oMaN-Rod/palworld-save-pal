import { palsData } from '$lib/data';
import { getAppState, getNavigationState } from '$states';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

const appState = getAppState();

export const addPalHandler: WSMessageHandler = {
	type: MessageType.ADD_PAL,
	async handle(data) {
		const { player_id, pal } = data;
		const nav = getNavigationState();

		if (!pal) {
			return;
		}

		if (appState.players && appState.players[player_id]?.pals) {
			const palData = palsData.pals[pal.character_key];
			pal.name = palData?.localized_name || pal.character_id;
			pal.elements = palData?.element_types || [];
			appState.players[player_id].pals[pal.instance_id] = pal;
			appState.selectedPal = pal;
			nav.activeTab = 'pal';
		}
	}
};

export const movePalHandler: WSMessageHandler = {
	type: MessageType.MOVE_PAL,
	async handle(data) {
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

export const palHandlers = [addPalHandler, movePalHandler];
