import type {
	SignalConfigUpdate,
	SignalGameDataCandidate,
	SignalSourceSelection,
	SignalStatus
} from '$types';
import { MessageType } from '$types';
import { sendAndWait } from '$utils/websocketUtils';

/**
 * Signal tab state. The backend answers every action with a full
 * `signal_status_update`, so one store field covers everything; the tab
 * polls on a 2s cadence while mounted, matching the rest of the app's
 * status pages.
 */
class SignalState {
	status = $state<SignalStatus | null>(null);
	candidates = $state<SignalGameDataCandidate[]>([]);
	loading = $state(false);
	saving = $state(false);

	#pollInterval: ReturnType<typeof setInterval> | null = null;

	startPolling(intervalMs = 2000): void {
		this.refresh();
		if (this.#pollInterval) return;
		this.#pollInterval = setInterval(() => this.refresh(), intervalMs);
	}

	stopPolling(): void {
		if (this.#pollInterval) {
			clearInterval(this.#pollInterval);
			this.#pollInterval = null;
		}
	}

	async refresh(): Promise<void> {
		if (!this.status) this.loading = true;
		// The status response arrives as a pushed signal_status_update; the
		// awaited reply is just an ack, so the store is filled by the handler.
		await sendAndWait(MessageType.GET_SIGNAL_STATUS).catch(() => null);
		this.loading = false;
	}

	async start(): Promise<void> {
		this.saving = true;
		await sendAndWait(MessageType.SIGNAL_START).catch(() => null);
		this.saving = false;
	}

	async stop(): Promise<void> {
		this.saving = true;
		await sendAndWait(MessageType.SIGNAL_STOP).catch(() => null);
		this.saving = false;
	}

	async updateConfig(update: SignalConfigUpdate): Promise<void> {
		this.saving = true;
		await sendAndWait(MessageType.UPDATE_SIGNAL_CONFIG, update).catch(() => null);
		this.saving = false;
	}

	async setSource(selection: SignalSourceSelection): Promise<void> {
		this.saving = true;
		await sendAndWait(MessageType.SET_SIGNAL_SOURCE, selection).catch(() => null);
		this.saving = false;
	}

	async clearSource(): Promise<void> {
		this.saving = true;
		await sendAndWait(MessageType.CLEAR_SIGNAL_SOURCE).catch(() => null);
		this.saving = false;
	}

	async regenerateToken(): Promise<void> {
		this.saving = true;
		await sendAndWait(MessageType.REGENERATE_SIGNAL_TOKEN).catch(() => null);
		this.saving = false;
	}

	async discoverGameData(): Promise<void> {
		await sendAndWait(MessageType.DISCOVER_SIGNAL_GAMEDATA).catch(() => null);
	}
}

let instance: SignalState | null = null;

export function getSignalState(): SignalState {
	if (!instance) {
		instance = new SignalState();
	}
	return instance;
}
