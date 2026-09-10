import { getSignalState } from '$states';
import type { SignalStatusJson } from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

function applyStatus(data: SignalStatusJson & { error?: string }) {
	if (data.error) return;
	getSignalState().status = data;
}

export const signalStatusHandler: WSMessageHandler = {
	type: MessageType.SIGNAL_STATUS,
	async handle(data) {
		applyStatus(data);
	}
};

export const signalSetSourceHandler: WSMessageHandler = {
	type: MessageType.SIGNAL_SET_SOURCE,
	async handle(data) {
		applyStatus(data);
	}
};

export const signalStartPairingHandler: WSMessageHandler = {
	type: MessageType.SIGNAL_START_PAIRING,
	async handle(data) {
		applyStatus(data);
	}
};

export const signalStopPairingHandler: WSMessageHandler = {
	type: MessageType.SIGNAL_STOP_PAIRING,
	async handle(data) {
		applyStatus(data);
	}
};

export const signalHandlers = [
	signalStatusHandler,
	signalSetSourceHandler,
	signalStartPairingHandler,
	signalStopPairingHandler
];
