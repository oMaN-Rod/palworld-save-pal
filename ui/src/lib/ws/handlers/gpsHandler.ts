import { palsData } from '$lib/data';
import { getAppState, getNavigationState } from '$states';
import { MessageType, type Pal } from '$types';
import type { WSMessageHandler } from '$ws/types';

export const getGpsPalsHandler: WSMessageHandler = {
	type: MessageType.GET_GPS_PALS,
	async handle(data: Record<string, Pal>, { goto }) {
		const appState = getAppState();
		appState.gps = data;
	}
};

export const addGpsPalHandler: WSMessageHandler = {
	type: MessageType.ADD_GPS_PAL,
	async handle(data) {
		const { pal, index } = data;
		const appState = getAppState();
		const nav = getNavigationState();

		if (!pal) {
			return;
		}

		if (appState.gps) {
			const palData = palsData.pals[pal.character_key];
			pal.name = palData?.localized_name || pal.character_id;
			pal.elements = palData?.element_types || [];
			if (appState.gps) {
				appState.gps[index] = pal;
			}
		}

		appState.selectedPal = pal;
		nav.activeTab = 'pal';
	}
};

export const gpsHandlers = [getGpsPalsHandler, addGpsPalHandler];
