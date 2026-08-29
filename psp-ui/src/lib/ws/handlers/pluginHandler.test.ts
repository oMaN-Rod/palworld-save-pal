import { beforeEach, describe, expect, it, vi } from 'vitest';

const { handleFrame, handleRequestReply, state } = vi.hoisted(() => ({
	handleFrame: vi.fn(),
	handleRequestReply: vi.fn(),
	state: { pluginId: 'user.one' as string | null }
}));

vi.mock('$lib/plugins/lspClient', () => ({
	lspClient: {
		handleFrame,
		handleRequestReply,
		get pluginId() {
			return state.pluginId;
		}
	}
}));

vi.mock('$lib/data', () => ({
	pluginsData: { finishRun: vi.fn(), plugins: [] }
}));

import { lspNotificationHandler, lspRequestHandler } from './pluginHandler';

beforeEach(() => {
	handleFrame.mockReset();
	handleRequestReply.mockReset();
	state.pluginId = 'user.one';
});

describe('lspRequestHandler', () => {
	it('hands the reply to the client, which correlates it by request_id', async () => {
		const reply = { request_id: 'abc-123', frame: { result: 'x' } };
		await lspRequestHandler.handle(reply, { goto: vi.fn() });
		expect(handleRequestReply).toHaveBeenCalledWith(reply);
	});

	it('answers under lsp_request so the dispatcher routes it here', () => {
		expect(lspRequestHandler.type).toBe('lsp_request');
	});
});

describe('lspNotificationHandler', () => {
	it('forwards a frame for the currently open plugin', async () => {
		const frame = { method: 'textDocument/publishDiagnostics', params: {} };
		await lspNotificationHandler.handle({ plugin_id: 'user.one', frame }, { goto: vi.fn() });
		expect(handleFrame).toHaveBeenCalledWith(frame);
	});

	it('drops a frame for a plugin that is not the one currently open', async () => {
		const frame = { method: 'textDocument/publishDiagnostics', params: {} };
		await lspNotificationHandler.handle({ plugin_id: 'user.other', frame }, { goto: vi.fn() });
		expect(handleFrame).not.toHaveBeenCalled();
	});

	it('logs and does not forward when the server reports an error', async () => {
		const error = console.error;
		console.error = vi.fn();
		try {
			await lspNotificationHandler.handle({ error: 'open it first' }, { goto: vi.fn() });
			expect(console.error).toHaveBeenCalled();
			expect(handleFrame).not.toHaveBeenCalled();
		} finally {
			console.error = error;
		}
	});

	it('does nothing when neither a frame nor an error is present', async () => {
		await lspNotificationHandler.handle({}, { goto: vi.fn() });
		expect(handleFrame).not.toHaveBeenCalled();
	});
});
