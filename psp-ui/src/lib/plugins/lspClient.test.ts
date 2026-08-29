import { beforeEach, describe, expect, it, vi } from 'vitest';

const sendAndWait = vi.fn();
const send = vi.fn();

vi.mock('$lib/utils/websocketUtils', () => ({
	sendAndWait: (type: unknown, data?: unknown) => sendAndWait(type, data),
	send: (type: unknown, data?: unknown) => send(type, data)
}));

import { MessageType } from '$types';
import { LspClient, positionToLsp, rangeFromLsp, severityFromLsp } from './lspClient';

beforeEach(() => {
	sendAndWait.mockReset();
	send.mockReset();
});

const ROOT_URI = 'file:///workspaces/user.demo';

async function openSession(
	client: LspClient,
	pluginId = 'user.demo',
	sources: Record<string, string> = { 'main.lua': 'return {}' }
): Promise<void> {
	sendAndWait.mockResolvedValueOnce({ root_uri: ROOT_URI });
	await client.open(pluginId, sources);
}

function sentRequestIds(): string[] {
	return send.mock.calls
		.filter(([type]) => type === MessageType.LSP_REQUEST)
		.map(([, data]) => data.request_id);
}

describe('coordinate conversion', () => {
	it('converts a 1-based Monaco position to a 0-based LSP position', () => {
		expect(positionToLsp(1, 1)).toEqual({ line: 0, character: 0 });
		expect(positionToLsp(12, 5)).toEqual({ line: 11, character: 4 });
	});

	it('converts a 0-based LSP range back to a 1-based Monaco range', () => {
		expect(
			rangeFromLsp({ start: { line: 0, character: 0 }, end: { line: 2, character: 6 } })
		).toEqual({
			startLineNumber: 1,
			startColumn: 1,
			endLineNumber: 3,
			endColumn: 7
		});
	});

	it('round-trips a position through both conversions', () => {
		const p = positionToLsp(9, 3);
		const r = rangeFromLsp({ start: p, end: p });
		expect(r.startLineNumber).toBe(9);
		expect(r.startColumn).toBe(3);
	});
});

describe('severity mapping', () => {
	it('maps the four LSP severities', () => {
		expect(severityFromLsp(1)).toBe('error');
		expect(severityFromLsp(2)).toBe('warning');
		expect(severityFromLsp(3)).toBe('info');
		expect(severityFromLsp(4)).toBe('hint');
	});

	it('treats an absent severity as an error, which is the LSP default', () => {
		expect(severityFromLsp(undefined)).toBe('error');
	});

	it('does not invent a severity for an out-of-range value', () => {
		expect(severityFromLsp(99)).toBe('error');
	});
});

describe('open', () => {
	it('adopts the workspace uri the server reports and builds document uris under it', async () => {
		const client = new LspClient();
		await openSession(client, 'user.demo', { 'lib/util.lua': 'return {}' });

		expect(sendAndWait).toHaveBeenCalledWith(MessageType.OPEN_LSP_SESSION, {
			plugin_id: 'user.demo'
		});
		expect(client.uriFor('lib/util.lua')).toBe(`${ROOT_URI}/lib/util.lua`);
		const [, data] = send.mock.calls[0];
		expect(data.frame.params.textDocument.uri).toBe(`${ROOT_URI}/lib/util.lua`);
	});

	it('refuses to open when the server could not start a session', async () => {
		sendAndWait.mockResolvedValueOnce({ error: 'the language server could not be installed' });
		const client = new LspClient();
		await expect(client.open('user.demo', { 'main.lua': 'return {}' })).rejects.toThrow(
			'the language server could not be installed'
		);
		expect(client.pluginId).toBeNull();
		expect(send).not.toHaveBeenCalled();
	});

	it('maps a uri the language server reported back to the source path it belongs to', async () => {
		const client = new LspClient();
		await openSession(client);

		expect(client.pathFor(`${ROOT_URI}/lib/util.lua`)).toBe('lib/util.lua');
		expect(client.pathFor(`${ROOT_URI}/a%20b.lua`)).toBe('a b.lua');
		expect(client.pathFor('file:///elsewhere/main.lua')).toBeNull();
	});

	it('percent-encodes a document uri the same way the server encoded the workspace', async () => {
		const client = new LspClient();
		await openSession(client, 'user.demo', { 'a b.lua': 'return {}' });
		expect(client.uriFor('a b.lua')).toBe(`${ROOT_URI}/a%20b.lua`);
	});
});

describe('completion', () => {
	it('requests textDocument/completion at the given position', async () => {
		const client = new LspClient();
		await openSession(client, 'user.demo');

		client.completion('main.lua', 3, 5);

		const [, data] = send.mock.calls.find(([type]) => type === MessageType.LSP_REQUEST)!;
		expect(data.frame.method).toBe('textDocument/completion');
		expect(data.frame.params).toEqual({
			textDocument: { uri: `${ROOT_URI}/main.lua` },
			position: { line: 2, character: 4 }
		});
	});
});

describe('requesting before a session is open', () => {
	it('rejects rather than throwing synchronously', async () => {
		const client = new LspClient();
		await expect(client.hover('main.lua', 1, 1)).rejects.toThrow(
			'LspClient.open() must be called before making requests'
		);
		expect(send).not.toHaveBeenCalled();
	});
});

describe('request errors', () => {
	it('rejects on a transport-level error', async () => {
		const client = new LspClient();
		await openSession(client, 'user.one');

		const hover = client.hover('main.lua', 1, 1);
		client.handleRequestReply({
			request_id: sentRequestIds()[0],
			error: 'no language server is running for user.one'
		});

		await expect(hover).rejects.toThrow('no language server is running for user.one');
	});

	it('rejects on a JSON-RPC-level error carried inside a successful frame', async () => {
		const client = new LspClient();
		await openSession(client, 'user.one');

		const hover = client.hover('main.lua', 1, 1);
		client.handleRequestReply({
			request_id: sentRequestIds()[0],
			frame: { jsonrpc: '2.0', id: 1, error: { code: -32601, message: 'method not found' } }
		});

		await expect(hover).rejects.toThrow('method not found');
	});

	it('resolves with frame.result when the frame carries no error', async () => {
		const client = new LspClient();
		await openSession(client, 'user.one');

		const hover = client.hover('main.lua', 1, 1);
		client.handleRequestReply({
			request_id: sentRequestIds()[0],
			frame: { jsonrpc: '2.0', id: 1, result: { contents: 'x' } }
		});

		await expect(hover).resolves.toEqual({ contents: 'x' });
	});

	it('rejects every request still in flight when the session is disposed', async () => {
		const client = new LspClient();
		await openSession(client, 'user.one');

		const hover = client.hover('main.lua', 1, 1);
		client.dispose();

		await expect(hover).rejects.toThrow('language server session');
	});

	it('leaves a request pending when a reply names an id it never sent', async () => {
		const client = new LspClient();
		await openSession(client, 'user.one');

		const hover = client.hover('main.lua', 1, 1);
		client.handleRequestReply({ request_id: 'never-sent', frame: { result: 'wrong' } });
		client.handleRequestReply({ request_id: sentRequestIds()[0], frame: { result: 'right' } });

		await expect(hover).resolves.toBe('right');
	});

	it('rejects a request still in flight when a different plugin is opened', async () => {
		const client = new LspClient();
		await openSession(client, 'user.one');

		const hover = client.hover('main.lua', 1, 1);
		await openSession(client, 'user.two');

		await expect(hover).rejects.toThrow('the language server session was replaced');
	});
});

describe('concurrent requests', () => {
	it('resolves each request with its own reply, not the most recent one', async () => {
		const client = new LspClient();
		await openSession(client, 'user.demo');

		const first = client.hover('main.lua', 1, 1);
		const second = client.hover('lib/util.lua', 2, 2);

		const ids = sentRequestIds();
		expect(ids).toHaveLength(2);
		expect(ids[0]).not.toEqual(ids[1]);

		client.handleRequestReply({ request_id: ids[1], frame: { result: 'second' } });
		client.handleRequestReply({ request_id: ids[0], frame: { result: 'first' } });

		expect(await first).toBe('first');
		expect(await second).toBe('second');
	});
});

describe('didChange versioning', () => {
	it('increments the document version on every change, starting after didOpen', async () => {
		const client = new LspClient();
		await openSession(client, 'user.one');

		client.didChange('main.lua', 'return 1');
		client.didChange('main.lua', 'return 2');

		const versions = send.mock.calls
			.filter(
				([type, data]) =>
					type === MessageType.LSP_NOTIFICATION && data.frame.method === 'textDocument/didChange'
			)
			.map(([, data]) => data.frame.params.textDocument.version);
		expect(versions).toEqual([2, 3]);
	});

	it('tracks each document path with its own version counter', async () => {
		const client = new LspClient();
		await openSession(client, 'user.one', { 'a.lua': 'a', 'b.lua': 'b' });

		client.didChange('a.lua', 'a2');

		const call = send.mock.calls.find(
			([type, data]) =>
				type === MessageType.LSP_NOTIFICATION &&
				data.frame.method === 'textDocument/didChange' &&
				data.frame.params.textDocument.uri.includes('a.lua')
		);
		expect(call?.[1].frame.params.textDocument.version).toBe(2);
	});
});

describe('handleFrame', () => {
	it('converts a publishDiagnostics notification into Monaco ranges and severities', () => {
		const client = new LspClient();
		const cb = vi.fn();
		client.onDiagnostics(cb);

		client.handleFrame({
			method: 'textDocument/publishDiagnostics',
			params: {
				uri: `${ROOT_URI}/main.lua`,
				diagnostics: [
					{
						range: { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } },
						severity: 2,
						message: 'unused variable'
					}
				]
			}
		});

		expect(cb).toHaveBeenCalledWith(`${ROOT_URI}/main.lua`, [
			{
				range: { startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 4 },
				severity: 'warning',
				message: 'unused variable'
			}
		]);
	});

	it('ignores a frame for a different method', () => {
		const client = new LspClient();
		const cb = vi.fn();
		client.onDiagnostics(cb);

		client.handleFrame({ method: 'window/logMessage', params: {} });

		expect(cb).not.toHaveBeenCalled();
	});

	it('does not throw and does not call the callback when params.diagnostics is missing', () => {
		const client = new LspClient();
		const cb = vi.fn();
		client.onDiagnostics(cb);

		expect(() =>
			client.handleFrame({ method: 'textDocument/publishDiagnostics', params: { uri: 'x' } })
		).not.toThrow();
		expect(cb).not.toHaveBeenCalled();
	});

	it('does not throw on a malformed, non-object frame', () => {
		const client = new LspClient();
		client.onDiagnostics(vi.fn());
		expect(() => client.handleFrame(null)).not.toThrow();
		expect(() => client.handleFrame('not an object')).not.toThrow();
	});
});
