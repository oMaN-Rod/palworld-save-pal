import { getSignalState } from '$states';
import type { SignalGameDataCandidate, SignalStatus } from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '$ws/types';

export const signalStatusHandler: WSMessageHandler = {
	type: MessageType.SIGNAL_STATUS_UPDATE,
	async handle(data: SignalStatus) {
		const state = getSignalState();
		state.status = data;
		state.loading = false;
		state.saving = false;
	}
};

export const discoverSignalGamedataHandler: WSMessageHandler = {
	type: MessageType.DISCOVER_SIGNAL_GAMEDATA,
	async handle(data: { candidates: SignalGameDataCandidate[] }) {
		getSignalState().candidates = data.candidates ?? [];
	}
};

export const signalHandlers = [signalStatusHandler, discoverSignalGamedataHandler];
