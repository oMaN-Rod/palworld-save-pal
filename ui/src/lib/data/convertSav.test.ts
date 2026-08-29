import { WorkerTransport } from '$lib/states/workerTransport.svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const socket = vi.hoisted(() => ({ state: null as unknown }));
vi.mock('$states/websocketState.svelte', () => ({ getSocketState: () => socket.state }));

import { jsonToSav, savToJson } from './convertSav';

class FakeWorker {
	onmessage: ((event: MessageEvent<unknown>) => void) | null = null;
	posted: any[] = [];
	transfers: Transferable[][] = [];
	reply: ((message: any) => unknown) | null = null;

	postMessage(data: unknown, transfer: Transferable[] = []) {
		this.posted.push(data);
		this.transfers.push(transfer);
		const answer = this.reply?.(data);
		if (answer !== undefined) {
			queueMicrotask(() => this.onmessage?.({ data: answer } as MessageEvent<unknown>));
		}
	}
	terminate() {}
}

let worker: FakeWorker;

function useWorkerTransport() {
	worker = new FakeWorker();
	const transport = new WorkerTransport({
		createWorker: () => worker as unknown as Worker,
		unloadTarget: null
	});
	transport.connect({ goto: async () => {} });
	socket.state = transport;
}

function useSocketTransport() {
	socket.state = { send: vi.fn(), sendAndWait: vi.fn() };
}

beforeEach(() => {
	vi.unstubAllGlobals();
});

describe('savToJson in the browser build', () => {
	it('converts through the worker engine and returns its JSON', async () => {
		useWorkerTransport();
		worker.reply = (message) => ({
			type: message.type,
			data: { json: '{"header":{}}' }
		});

		const json = await savToJson(new Uint8Array([1, 2, 3]), 'Level.sav');

		expect(json).toBe('{"header":{}}');
		expect(worker.posted[0].type).toBe('convert_sav_to_json');
		expect(worker.posted[0].bytes).toEqual(new Uint8Array([1, 2, 3]));
	});

	// The bytes are a whole save; copying them into a JSON frame is what the
	// binary hand-off exists to avoid.
	it('transfers the buffer rather than cloning it', async () => {
		useWorkerTransport();
		const bytes = new Uint8Array([1, 2, 3]);
		worker.reply = (message) => ({ type: message.type, data: { json: '{}' } });

		await savToJson(bytes, 'Level.sav');

		expect(worker.transfers[0]).toEqual([bytes.buffer]);
	});

	// A file the engine cannot parse must reach the caller as an error. Silently
	// resolving would drop the editor back to the dropzone with no explanation.
	it('rejects with the engine error when the file will not parse', async () => {
		useWorkerTransport();
		worker.reply = (message) => ({ type: message.type, data: { error: 'not a GVAS file' } });

		await expect(savToJson(new Uint8Array([0]), 'bad.sav')).rejects.toThrow('not a GVAS file');
	});
});

describe('jsonToSav in the browser build', () => {
	it('returns the bytes the engine wrote', async () => {
		useWorkerTransport();
		worker.reply = (message) => ({
			type: message.type,
			data: { bytes: new Uint8Array([4, 5]) }
		});

		const bytes = await jsonToSav('{"header":{}}');

		expect(bytes).toEqual(new Uint8Array([4, 5]));
		expect(worker.posted[0]).toEqual({ type: 'convert_json_to_sav', json: '{"header":{}}' });
	});

	it('rejects with the engine error when the JSON is not a save', async () => {
		useWorkerTransport();
		worker.reply = (message) => ({ type: message.type, data: { error: 'missing field `header`' } });

		await expect(jsonToSav('{}')).rejects.toThrow('missing field `header`');
	});
});

describe('the desktop and Docker builds', () => {
	it('posts the file to the backend converter', async () => {
		useSocketTransport();
		const fetchMock = vi.fn(async (_url: string) => new Response('{"header":{}}', { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);

		const json = await savToJson(new Uint8Array([1]), 'Level.sav');

		expect(json).toBe('{"header":{}}');
		expect(fetchMock.mock.calls[0][0]).toBe('/api/convert/sav-to-json');
	});

	it('reports a backend failure instead of returning the error page', async () => {
		useSocketTransport();
		vi.stubGlobal(
			'fetch',
			vi.fn(async (_url: string) => new Response('nope', { status: 405 }))
		);

		await expect(savToJson(new Uint8Array([1]), 'Level.sav')).rejects.toThrow('405');
	});
});
