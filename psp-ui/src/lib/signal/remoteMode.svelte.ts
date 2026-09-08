import { setTransportDelegate, resetTransportDelegate } from '$lib/states/websocketState.svelte';
import type { Transport } from '$lib/ws/types';
import { RemoteTransport } from './remoteTransport.svelte';
import type { SignalSession } from './session.svelte';
import { getWebSignalSession } from './webSession';

export interface RemoteModeDeps {
	getSession: () => SignalSession;
	createTransport: (session: SignalSession) => RemoteTransport;
	setTransportDelegate: (transport: Transport) => void;
	resetTransportDelegate: () => void;
}

function defaultDeps(): RemoteModeDeps {
	return {
		getSession: getWebSignalSession,
		createTransport: (session) => new RemoteTransport(session),
		setTransportDelegate,
		resetTransportDelegate
	};
}

export class RemoteModeState {
	#deps: RemoteModeDeps;
	#active = $state(false);
	#transport = $state.raw<RemoteTransport | null>(null);

	constructor(deps?: Partial<RemoteModeDeps>) {
		this.#deps = { ...defaultDeps(), ...deps };
	}

	get active(): boolean {
		return this.#active;
	}

	get transport(): RemoteTransport | null {
		return this.#transport;
	}

	enter(): void {
		if (this.#active) return;
		const session = this.#deps.getSession();
		if (!session.connected) {
			throw new Error('RemoteMode: cannot enter while the signal session is disconnected');
		}
		const transport = this.#deps.createTransport(session);
		this.#deps.setTransportDelegate(transport);
		this.#transport = transport;
		this.#active = true;
	}

	exit(): void {
		if (!this.#active) return;
		this.#transport?.dispose();
		this.#deps.resetTransportDelegate();
		this.#transport = null;
		this.#active = false;
	}
}

let instance: RemoteModeState | undefined;

export function getRemoteMode(): RemoteModeState {
	if (!instance) instance = new RemoteModeState();
	return instance;
}
