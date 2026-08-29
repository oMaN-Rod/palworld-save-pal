import { palsData } from '$lib/data';
import { getAppState, getNavigationState } from '$states';
import { MessageType, type Pal } from '$types';
import type { WSMessageHandler } from '$ws/types';

export const getGpsPalsHandler: WSMessageHandler = {
	type: MessageType.GET_GPS_PALS,
	async handle(data: Record<string, Pal>, { goto }) {
		const appState = getAppState();
		appState.gps = data;
		appState.gpsLoaded = true;
		appState.loadingGps = false;
	}
};

export const getGpsResponseHandler: WSMessageHandler = {
	type: MessageType.GET_GPS_RESPONSE,
	async handle(
		data: Record<string, Pal> | { error?: string; available?: boolean; message?: string }
	) {
		const appState = getAppState();
		appState.loadingGps = false;

		if ('error' in data || 'available' in data) {
			console.log('GPS not available:', data);
			appState.gpsLoaded = true;
			return;
		}

		appState.gps = data as Record<string, Pal>;
		appState.gpsLoaded = true;
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

export const gpsHandlers = [getGpsPalsHandler, getGpsResponseHandler, addGpsPalHandler];
