import { afterEach, describe, expect, it } from 'vitest';
import type { Transport, WSHandlerContext } from '$lib/ws/types';
import { getSocketState, resetTransportDelegate, setTransportDelegate } from './websocketState.svelte';

function fakeTransport(kind: Transport['kind']) {
	return {
		kind,
		calls: [] as string[],
		lastContext: null as WSHandlerContext | null,
		message: null as Transport['message'],
		connected: false,
		connect(context: WSHandlerContext) {
			this.calls.push('connect');
			this.lastContext = context;
		},
		isConnected() {
			return this.connected;
		},
		async send(messageData: string) {
			this.calls.push(`send:${messageData}`);
		},
		async sendBytes(type: string, bytes: Uint8Array) {
			this.calls.push(`sendBytes:${type}:${bytes.length}`);
		},
		async sendAndWait(messageData: any) {
			this.calls.push(`sendAndWait:${messageData.type}`);
			return { type: messageData.type };
		},
		clear(messageType: string) {
			this.calls.push(`clear:${messageType}`);
		}
	};
}

afterEach(() => {
	resetTransportDelegate();
});

describe('transport facade', () => {
	it('forwards calls to the current delegate', async () => {
		const fake = fakeTransport('remote');
		setTransportDelegate(fake as unknown as Transport);
		const facade = getSocketState();

		expect(facade.kind).toBe('remote');

		await facade.send('hello');
		await facade.sendBytes('t', new Uint8Array([1, 2]));
		await facade.sendAndWait({ type: 'ping' });
		facade.clear('ping');
		facade.message = { type: 'x', data: {} } as any;

		expect(facade.message).toEqual({ type: 'x', data: {} });
		expect(fake.calls).toEqual(['send:hello', 'sendBytes:t:2', 'sendAndWait:ping', 'clear:ping']);
	});

	it('does not call connect on a new delegate when no context was ever captured', () => {
		const fake = fakeTransport('remote');
		setTransportDelegate(fake as unknown as Transport);

		expect(fake.calls).toEqual([]);
	});

	it('switches which delegate receives calls and connects the new one with the captured context', () => {
		const first = fakeTransport('remote');
		const second = fakeTransport('worker');
		const context: WSHandlerContext = { goto: (async () => {}) as WSHandlerContext['goto'] };

		setTransportDelegate(first as unknown as Transport);
		const facade = getSocketState();
		facade.connect(context);

		expect(first.calls).toContain('connect');
		expect(first.lastContext).toBe(context);

		setTransportDelegate(second as unknown as Transport);

		expect(second.calls).toContain('connect');
		expect(second.lastContext).toBe(context);
		expect(facade.kind).toBe('worker');

		facade.send('to-second');

		expect(second.calls).toContain('send:to-second');
		expect(first.calls.some((c) => c.startsWith('send'))).toBe(false);
	});

	it('restores the boot delegate on reset', () => {
		const bootKind = getSocketState().kind;
		const fake = fakeTransport('remote');
		setTransportDelegate(fake as unknown as Transport);

		expect(getSocketState().kind).toBe('remote');

		resetTransportDelegate();

		expect(getSocketState().kind).toBe(bootKind);
	});
});
