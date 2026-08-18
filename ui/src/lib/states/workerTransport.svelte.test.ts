import { beforeEach, describe, expect, it } from 'vitest';
import { WorkerTransport } from './workerTransport.svelte';

class FakeWorker {
	terminated = false;
	posted: unknown[] = [];
	transfers: Transferable[][] = [];
	onmessage: ((event: MessageEvent<unknown>) => void) | null = null;
	postMessage(data: unknown, transfer: Transferable[] = []) {
		this.posted.push(data);
		this.transfers.push(transfer);
	}
	terminate() {
		this.terminated = true;
	}
}

class FakeUnloadTarget {
	listeners: Array<(event: PageTransitionEvent) => void> = [];
	addEventListener(_type: string, listener: EventListener) {
		this.listeners.push(listener as (event: PageTransitionEvent) => void);
	}
	removeEventListener(_type: string, listener: EventListener) {
		this.listeners = this.listeners.filter((l) => l !== listener);
	}
	hide(persisted: boolean) {
		for (const listener of [...this.listeners]) {
			listener({ persisted } as PageTransitionEvent);
		}
	}
}

let workers: FakeWorker[] = [];
let unload: FakeUnloadTarget;

function transport() {
	return new WorkerTransport({
		createWorker: () => {
			const worker = new FakeWorker();
			workers.push(worker);
			return worker as unknown as Worker;
		},
		unloadTarget: unload
	});
}

beforeEach(() => {
	workers = [];
	unload = new FakeUnloadTarget();
});

describe('WorkerTransport', () => {
	// The worker's sqlite holds exclusive OPFS access handles for its whole
	// lifetime. Leaving it running past the page means the next page cannot
	// acquire the pool and silently runs on a non-persistent in-memory database.
	it('terminates the worker when the page is discarded', () => {
		const ws = transport();
		ws.connect({ goto: async () => {} });

		unload.hide(false);

		expect(workers[0].terminated).toBe(true);
		expect(ws.isConnected()).toBe(false);
	});

	it('keeps the worker when the page is only being cached for a back-navigation', () => {
		const ws = transport();
		ws.connect({ goto: async () => {} });

		unload.hide(true);

		expect(workers[0].terminated).toBe(false);
		expect(ws.isConnected()).toBe(true);
	});

	it('does not start a second worker when connect is called again', () => {
		const ws = transport();
		ws.connect({ goto: async () => {} });
		ws.connect({ goto: async () => {} });

		expect(workers).toHaveLength(1);
	});

	it('stops listening for unload once the worker is gone', () => {
		const ws = transport();
		ws.connect({ goto: async () => {} });

		unload.hide(false);

		expect(unload.listeners).toHaveLength(0);
	});

	// A save zip encoded as a JSON number array costs several times its own size
	// in string, and the decompressed save behind it cannot be a string at all.
	it('hands save bytes to the worker as a transferred buffer, not JSON', async () => {
		const ws = transport();
		ws.connect({ goto: async () => {} });
		const bytes = new Uint8Array([80, 75, 3, 4]);

		await ws.sendBytes('load_zip_file', bytes);

		expect(workers[0].posted[0]).toEqual({ type: 'load_zip_file', bytes });
		expect(workers[0].transfers[0]).toEqual([bytes.buffer]);
	});

	it('dispatches a binary message from the worker without parsing it', async () => {
		const ws = transport();
		const dispatched: unknown[] = [];
		ws.connect({ goto: async () => {} });
		const frame = {
			type: 'download_save_file',
			data: [{ name: 'W.zip', bytes: new Uint8Array([1]) }]
		};

		await workers[0].onmessage?.({ data: frame } as MessageEvent<unknown>);

		expect(ws.message).toEqual(frame);
		void dispatched;
	});
});
