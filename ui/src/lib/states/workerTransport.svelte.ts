import { getDispatcher } from '$lib/ws/dispatcher';
import type { WSHandlerContext } from '$lib/ws/types';
import type { Message } from '$types';

export class WorkerTransport {
	#worker: Worker | null = null;
	#message = $state<Message | null>(null);
	#connected = $state(false);
	#dispatcher = getDispatcher();
	#queue = new Map<string, (value: unknown) => void>();

	connect(context: WSHandlerContext) {
		// Relative URL (not `$lib`) so Vite's worker plugin resolves it; this file
		// is ui/src/lib/states, the worker is ui/src/lib/worker.
		this.#worker = new Worker(new URL('../worker/psp.worker.ts', import.meta.url), {
			type: 'module'
		});
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
