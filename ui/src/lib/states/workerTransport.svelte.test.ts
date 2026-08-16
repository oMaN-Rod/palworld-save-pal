import { beforeEach, describe, expect, it } from 'vitest';
import { WorkerTransport } from './workerTransport.svelte';

class FakeWorker {
	terminated = false;
	posted: string[] = [];
	onmessage: ((event: MessageEvent<string>) => void) | null = null;
	postMessage(data: string) {
		this.posted.push(data);
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
});
