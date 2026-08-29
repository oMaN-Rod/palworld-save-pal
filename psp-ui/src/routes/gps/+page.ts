import { getAppState } from '$states';
import type { PageLoad } from './$types';

export const load: PageLoad = async () => {
	const appState = getAppState();

	if (!appState.gpsLoaded && appState.hasGpsAvailable) {
		await appState.loadGpsLazy();
	}

	return {
		gpsLoaded: appState.gpsLoaded,
		hasGpsAvailable: appState.hasGpsAvailable
	};
};
