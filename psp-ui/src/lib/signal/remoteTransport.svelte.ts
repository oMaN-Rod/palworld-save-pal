import { getDispatcher } from '$lib/ws/dispatcher';
import type { Transport, WSHandlerContext } from '$lib/ws/types';
import type { Message } from '$types';
import { ChunkAssembler, chunkFrame } from './ctlFraming';
import type { SignalSession } from './session.svelte';

const REMOTE_SEND_AND_WAIT_TIMEOUT_MS = 30_000;

export class RemoteTransport implements Transport {
	readonly kind = 'remote' as const;
	#session: SignalSession;
	#dispatcher = getDispatcher();
	#context: WSHandlerContext | null = null;
	#unsubscribe: (() => void) | null = null;
	#assembler = new ChunkAssembler();
	#nextChunkId = 0;
	#waiters = new Map<
		string,
		{ token: number; resolve: (value: unknown) => void; reject: (err: unknown) => void }
	>();
	#waiterToken = 0;
	#message = $state.raw<Message | null>(null);
	#lastSessionId = $state<string | null>(null);

	constructor(session: SignalSession) {
		this.#session = session;
	}

	connect(context: WSHandlerContext): void {
		this.#context = context;
		if (this.#unsubscribe) return;
		this.#unsubscribe = this.#session.message.subscribe((envelope) => this.#onFrame(envelope));
	}

	isConnected(): boolean {
		return this.#session.connected;
	}

	async send(messageData: string): Promise<void> {
		for (const piece of chunkFrame(messageData, () => this.#nextChunkId++)) {
			this.#session.sendRaw(piece);
		}
	}

	async sendBytes(_type: string, _bytes: Uint8Array): Promise<void> {
		throw new Error('File transfer is not available over a remote session');
	}

	sendAndWait(messageData: any): Promise<any> {
		const type = messageData.type;
		const token = ++this.#waiterToken;
		return new Promise((resolve, reject) => {
			const timer = setTimeout(() => {
				if (this.#waiters.get(type)?.token === token) this.#waiters.delete(type);
				reject(new Error('timeout: ' + type));
			}, REMOTE_SEND_AND_WAIT_TIMEOUT_MS);
			this.#waiters.set(type, {
				token,
				resolve: (value) => {
					clearTimeout(timer);
					resolve(value);
				},
				reject: (err) => {
					clearTimeout(timer);
					reject(err);
				}
			});
			this.send(JSON.stringify(messageData)).catch((err) => {
				clearTimeout(timer);
				if (this.#waiters.get(type)?.token === token) this.#waiters.delete(type);
				reject(err);
			});
		});
	}

	clear(messageType: string): void {
		if (this.#message?.type === messageType) this.#message = null;
	}

	resetFraming(): void {
		this.#assembler = new ChunkAssembler();
	}

	dispose(): void {
		if (this.#unsubscribe) {
			this.#unsubscribe();
			this.#unsubscribe = null;
		}
		this.resetFraming();
		const reason = new Error('RemoteTransport disposed');
		for (const waiter of this.#waiters.values()) waiter.reject(reason);
		this.#waiters.clear();
		this.#context = null;
	}

	get message(): Message | null {
		return this.#message;
	}

	set message(value: Message | null) {
		this.#message = value;
	}

	get connected(): boolean {
		return this.#session.connected;
	}

	get lastSessionId(): string | null {
		return this.#lastSessionId;
	}

	#onFrame(envelope: { type: string; data?: unknown }): void {
		const outcome = this.#assembler.accept(envelope);
		if (outcome.kind === 'pending' || outcome.kind === 'rejected') return;
		if (outcome.kind === 'not-chunk') {
			this.#deliver(outcome.envelope as { type: string; data: unknown });
			return;
		}
		try {
			this.#deliver(JSON.parse(outcome.text));
		} catch {
		}
	}

	async #deliver(data: { type: string; data?: unknown }): Promise<void> {
		if (data.type === 'loaded_save_files') {
			const sessionId = (data.data as { session_id?: unknown } | undefined)?.session_id;
			if (typeof sessionId === 'string') this.#lastSessionId = sessionId;
		}

		const waiter = this.#waiters.get(data.type);
		if (waiter) {
			this.#waiters.delete(data.type);
			waiter.resolve(data);
			return;
		}

		this.#message = data as Message;
		if (this.#context) await this.#dispatcher.dispatch(data as Message, this.#context);
	}
}
