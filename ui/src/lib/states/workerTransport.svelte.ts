import { getDispatcher } from '$lib/ws/dispatcher';
import type { WSHandlerContext } from '$lib/ws/types';
import type { Message } from '$types';

type UnloadTarget = Pick<EventTarget, 'addEventListener' | 'removeEventListener'>;

export type WorkerTransportOptions = {
	createWorker?: () => Worker;
	unloadTarget?: UnloadTarget | null;
};

export class WorkerTransport {
	#worker: Worker | null = null;
	#message = $state<Message | null>(null);
	#connected = $state(false);
	#dispatcher = getDispatcher();
	#queue = new Map<string, (value: unknown) => void>();
	#createWorker: () => Worker;
	#unloadTarget: UnloadTarget | null;

	constructor(options: WorkerTransportOptions = {}) {
		this.#createWorker =
			options.createWorker ??
			// Relative URL (not `$lib`) so Vite's worker plugin resolves it; this
			// file is ui/src/lib/states, the worker is ui/src/lib/worker.
			(() => new Worker(new URL('../worker/psp.worker.ts', import.meta.url), { type: 'module' }));
		this.#unloadTarget =
			options.unloadTarget !== undefined
				? options.unloadTarget
				: typeof globalThis.addEventListener === 'function'
					? globalThis
					: null;
	}

	// The worker's sqlite holds exclusive OPFS access handles for its whole
	// lifetime, and only one holder per origin can have them. Leaving it running
	// past the page leaves the next one unable to acquire the pool, silently
	// downgrading it to a non-persistent in-memory database. `persisted` filters
	// out a bfcache suspension, where the page — and this worker — come back.
	#onPageHide = (event: Event) => {
		if ((event as PageTransitionEvent).persisted) return;
		this.disconnect();
	};

	connect(context: WSHandlerContext) {
		if (this.#worker) return;
		this.#worker = this.#createWorker();
		this.#unloadTarget?.addEventListener('pagehide', this.#onPageHide);
		this.#worker.onmessage = async (event: MessageEvent<string>) => {
			const data = JSON.parse(event.data);
			if (!data) return;
			if (data.type && this.#queue.has(data.type)) {
				this.#queue.get(data.type)!(data);
				this.#queue.delete(data.type);
				return;
			}
			this.#message = data;
			await this.#dispatcher.dispatch(data, context);
		};
		this.#connected = true;
	}

	disconnect() {
		this.#unloadTarget?.removeEventListener('pagehide', this.#onPageHide);
		this.#worker?.terminate();
		this.#worker = null;
		this.#connected = false;
	}

	isConnected(): boolean {
		return this.#connected;
	}

	async send(messageData: string) {
		this.#worker?.postMessage(messageData);
	}

	async sendAndWait(messageData: any): Promise<any> {
		return new Promise((resolve) => {
			this.#queue.set(messageData.type, resolve);
			this.send(JSON.stringify(messageData));
		});
	}

	clear(messageType: string) {
		if (this.#message?.type === messageType) this.#message = null;
	}
	get message() {
		return this.#message;
	}
	set message(v: Message | null) {
		this.#message = v;
	}
	get connected() {
		return this.#connected;
	}
}
