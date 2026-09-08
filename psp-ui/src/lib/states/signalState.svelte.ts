import type {
	SetSignalSourceRequest,
	SignalDeviceJson,
	SignalDeviceListJson,
	SignalStartPairingJson,
	SignalStatusJson
} from '$types';
import { MessageType } from '$types';
import { sendAndWait } from '$utils/websocketUtils';

type SignalReply<T> = (T & { error?: undefined }) | { error: string };

export class SignalState {
	status = $state<SignalStatusJson | null>(null);
	url = $state<string | null>(null);
	devices = $state<SignalDeviceJson[]>([]);

	#statusGeneration = 0;
	#devicesGeneration = 0;

	get armed(): boolean {
		return this.status?.armed ?? false;
	}

	#applyIfStatusCurrent(generation: number, apply: () => void): void {
		if (generation === this.#statusGeneration) apply();
	}

	#applyIfDevicesCurrent(generation: number, apply: () => void): void {
		if (generation === this.#devicesGeneration) apply();
	}

	async refresh(): Promise<void> {
		const generation = ++this.#statusGeneration;
		try {
			const response = await sendAndWait<SignalStatusJson>(MessageType.SIGNAL_STATUS);
			this.#applyIfStatusCurrent(generation, () => {
				this.status = response;
			});
		} catch (error) {
			console.error('signal_status failed', error);
		}
	}

	async setSource(selection: SetSignalSourceRequest): Promise<void> {
		const generation = ++this.#statusGeneration;
		const response = await sendAndWait<SignalReply<SignalStatusJson>>(
			MessageType.SIGNAL_SET_SOURCE,
			selection
		);
		if ('error' in response) throw new Error(response.error);
		this.#applyIfStatusCurrent(generation, () => {
			this.status = response;
		});
	}

	async startPairing(): Promise<void> {
		const generation = ++this.#statusGeneration;
		const response = await sendAndWait<SignalReply<SignalStartPairingJson>>(
			MessageType.SIGNAL_START_PAIRING
		);
		if ('error' in response) throw new Error(response.error);
		this.#applyIfStatusCurrent(generation, () => {
			this.status = response;
			this.url = response.url ?? null;
		});
	}

	async stopPairing(): Promise<void> {
		const generation = ++this.#statusGeneration;
		const response = await sendAndWait<SignalReply<SignalStatusJson>>(
			MessageType.SIGNAL_STOP_PAIRING
		);
		if ('error' in response) throw new Error(response.error);
		this.#applyIfStatusCurrent(generation, () => {
			this.status = response;
			this.url = null;
		});
	}

	async setArmed(armed: boolean): Promise<void> {
		const generation = ++this.#statusGeneration;
		const response = await sendAndWait<SignalReply<SignalStatusJson>>(
			MessageType.SIGNAL_SET_ARMED,
			{ armed }
		);
		if ('error' in response) throw new Error(response.error);
		this.#applyIfStatusCurrent(generation, () => {
			this.status = response;
		});
	}

	async listDevices(): Promise<void> {
		const generation = ++this.#devicesGeneration;
		const response = await sendAndWait<SignalReply<SignalDeviceListJson>>(
			MessageType.SIGNAL_LIST_DEVICES
		);
		if ('error' in response) throw new Error(response.error);
		this.#applyIfDevicesCurrent(generation, () => {
			this.devices = response.devices;
		});
	}

	async renameDevice(deviceId: string, name: string): Promise<void> {
		const generation = ++this.#devicesGeneration;
		const response = await sendAndWait<SignalReply<SignalDeviceListJson>>(
			MessageType.SIGNAL_RENAME_DEVICE,
			{ device_id: deviceId, name }
		);
		if ('error' in response) throw new Error(response.error);
		this.#applyIfDevicesCurrent(generation, () => {
			this.devices = response.devices;
		});
	}

	async revokeDevice(deviceId: string): Promise<void> {
		const generation = ++this.#devicesGeneration;
		const response = await sendAndWait<SignalReply<SignalDeviceListJson>>(
			MessageType.SIGNAL_REVOKE_DEVICE,
			{ device_id: deviceId }
		);
		if ('error' in response) throw new Error(response.error);
		this.#applyIfDevicesCurrent(generation, () => {
			this.devices = response.devices;
		});
	}

	async resetRemoteAccess(): Promise<void> {
		const statusGeneration = ++this.#statusGeneration;
		const devicesGeneration = ++this.#devicesGeneration;
		const response = await sendAndWait<SignalReply<SignalStatusJson>>(
			MessageType.SIGNAL_RESET_REMOTE_ACCESS
		);
		if ('error' in response) throw new Error(response.error);
		this.#applyIfStatusCurrent(statusGeneration, () => {
			this.status = response;
		});
		this.#applyIfDevicesCurrent(devicesGeneration, () => {
			this.devices = [];
		});
	}
}

let signalStateInstance: SignalState | undefined;

export function getSignalState(): SignalState {
	if (!signalStateInstance) {
		signalStateInstance = new SignalState();
	}
	return signalStateInstance;
}
