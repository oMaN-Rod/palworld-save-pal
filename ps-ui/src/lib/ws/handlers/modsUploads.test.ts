import { beforeEach, describe, expect, it, vi } from 'vitest';

const { send, toast, holder } = vi.hoisted(() => ({
	send: vi.fn(),
	toast: { add: vi.fn() },
	holder: { state: undefined as unknown, server: undefined as unknown }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn()
}));

vi.mock('$i18n/messages', () => ({
	mods_panel_title: () => 'Mods',
	mods_panel_applied: () => 'Mods applied'
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	const { ServerState } = await import('$lib/states/serverState.svelte');
	holder.state = new ModsState();
	holder.server = new ServerState();
	return {
		getModsState: () => holder.state,
		getServerState: () => holder.server,
		getToastState: () => toast
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import { MAX_UPLOAD_BYTES, bytesToBase64, sha256Hex } from '$lib/utils/modUpload';
import { modUploadBeginHandler, modUploadChunkHandler, modUploadEndHandler } from './modsHandler';

const context = { goto: vi.fn() };
let state: ModsState;
const bytes = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
const file = () => new File([bytes], 'Cool Mod.zip');
const sent = (type: string) =>
	send.mock.calls.filter(([sentType]) => sentType === type).map(([, data]) => data);

beforeEach(async () => {
	await import('$states');
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	state = holder.state as ModsState;
	send.mockReset();
	toast.add.mockReset();
});

async function begun(chunkSize = 4) {
	await state.startUpload('client-a', file(), 'install');
	await modUploadBeginHandler.handle(
		{ upload_id: 'u1', chunk_size: chunkSize, name: 'Cool Mod.zip' },
		context
	);
	await vi.waitFor(() => expect(sent('mod_upload_chunk')).toHaveLength(1));
}

describe('uploads', () => {
	it('hashes the file, begins, and sends one chunk per reply until it ends', async () => {
		await state.startUpload('client-a', file(), 'install');
		expect(sent('mod_upload_begin')).toEqual([
			{ name: 'Cool Mod.zip', size: 10, sha256: await sha256Hex(file()) }
		]);
		expect(state.uploadStatus).toMatchObject({
			stage: 'uploading',
			purpose: 'install',
			targetId: 'client-a',
			sent: 0,
			uploadId: null
		});

		await modUploadBeginHandler.handle(
			{ upload_id: 'u1', chunk_size: 4, name: 'Cool Mod.zip' },
			context
		);
		await vi.waitFor(() => expect(sent('mod_upload_chunk')).toHaveLength(1));
		expect(sent('mod_upload_chunk')[0]).toEqual({
			upload_id: 'u1',
			seq: 0,
			data_b64: bytesToBase64(bytes.subarray(0, 4))
		});

		await modUploadChunkHandler.handle({ upload_id: 'u1', seq: 0, received: 4 }, context);
		await vi.waitFor(() => expect(sent('mod_upload_chunk')).toHaveLength(2));
		expect(state.uploadStatus?.sent).toBe(4);

		await modUploadChunkHandler.handle({ upload_id: 'u1', seq: 1, received: 8 }, context);
		await vi.waitFor(() => expect(sent('mod_upload_chunk')).toHaveLength(3));
		expect(sent('mod_upload_chunk')[2]).toEqual({
			upload_id: 'u1',
			seq: 2,
			data_b64: bytesToBase64(bytes.subarray(8))
		});

		await modUploadChunkHandler.handle({ upload_id: 'u1', seq: 2, received: 10 }, context);
		await vi.waitFor(() => expect(sent('mod_upload_end')).toEqual([{ upload_id: 'u1' }]));
		expect(state.uploadStatus?.stage).toBe('finishing');

		await modUploadEndHandler.handle(
			{ upload_id: 'u1', path: 'C:/app/downloads/Cool Mod.zip' },
			context
		);
		expect(state.uploadStatus).toMatchObject({
			stage: 'done',
			path: 'C:/app/downloads/Cool Mod.zip',
			sent: 10
		});
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('never has two chunks outstanding', async () => {
		await begun();
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(sent('mod_upload_chunk')).toHaveLength(1);
	});

	it('ignores replies for another upload', async () => {
		await begun();
		await modUploadChunkHandler.handle({ upload_id: 'other', seq: 0, received: 4 }, context);
		await modUploadEndHandler.handle({ upload_id: 'other', path: 'C:/x.zip' }, context);
		await modUploadChunkHandler.handle(
			{ upload_id: 'other', seq: 0, error: { code: 'upload_not_found', message: 'gone' } },
			context
		);
		await new Promise((resolve) => setTimeout(resolve, 20));

		expect(sent('mod_upload_chunk')).toHaveLength(1);
		expect(state.uploadStatus).toMatchObject({ stage: 'uploading', uploadId: 'u1', error: null });
	});

	it('stops at a refusal and keeps it on the upload', async () => {
		await begun();
		await modUploadChunkHandler.handle(
			{
				upload_id: 'u1',
				seq: 0,
				error: { code: 'chunk_out_of_order', message: 'expected 0', expected_seq: 0 }
			},
			context
		);
		await modUploadChunkHandler.handle({ upload_id: 'u1', seq: 0, received: 4 }, context);
		await new Promise((resolve) => setTimeout(resolve, 20));

		expect(state.uploadStatus).toMatchObject({
			stage: 'failed',
			error: { code: 'chunk_out_of_order' }
		});
		expect(state.uploading).toBe(false);
		expect(sent('mod_upload_chunk')).toHaveLength(1);
		expect(toast.add).not.toHaveBeenCalled();
		expect(state.lastError).toEqual({});
	});

	it('takes a begin refusal, which names no upload id', async () => {
		await state.startUpload('client-a', file(), 'install');
		await modUploadBeginHandler.handle(
			{ name: 'Cool Mod.zip', error: { code: 'invalid_name', message: 'bad name' } },
			context
		);
		expect(state.uploadStatus).toMatchObject({ stage: 'failed', error: { code: 'invalid_name' } });
	});

	it('refuses a file over the size cap without sending anything', async () => {
		const huge = { name: 'big.zip', size: MAX_UPLOAD_BYTES + 1 } as File;
		await state.startUpload('client-a', huge, 'install');
		expect(send).not.toHaveBeenCalled();
		expect(state.uploadStatus).toMatchObject({
			stage: 'failed',
			error: { code: 'upload_too_large', max_bytes: MAX_UPLOAD_BYTES }
		});
	});

	it('sends nothing once cleared, even while hashing', async () => {
		const started = state.startUpload('client-a', file(), 'install');
		state.clearUpload();
		await started;
		expect(send).not.toHaveBeenCalled();
		expect(state.uploadStatus).toBeNull();
	});

	it('sends no further chunk after a cancel', async () => {
		await begun();
		state.clearUpload();
		await modUploadChunkHandler.handle({ upload_id: 'u1', seq: 0, received: 4 }, context);
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(sent('mod_upload_chunk')).toHaveLength(1);
		expect(sent('mod_upload_end')).toEqual([]);
	});

	it('runs one upload at a time', async () => {
		await state.startUpload('client-a', file(), 'install');
		await state.startUpload('client-a', file(), 'import');
		expect(sent('mod_upload_begin')).toHaveLength(1);
		expect(state.uploadStatus?.purpose).toBe('install');
	});

	it('fails an upload the connection dropped, and forgets it on reset', async () => {
		state.connectionChanged(true);
		await begun();
		state.connectionChanged(false);
		expect(state.uploadStatus).toMatchObject({
			stage: 'failed',
			error: { code: 'connection_lost' }
		});

		state.reset();
		expect(state.uploadStatus).toBeNull();
	});

	it("ignores a cancelled upload's begin refusal while the next upload is hashing", async () => {
		await state.startUpload('client-a', file(), 'install');
		state.clearUpload();
		const next = state.startUpload('client-a', file(), 'install');
		expect(state.uploadStatus?.stage).toBe('hashing');

		await modUploadBeginHandler.handle(
			{ name: 'Cool Mod.zip', error: { code: 'invalid_name', message: 'bad name' } },
			context
		);
		await next;

		expect(state.uploadStatus).toMatchObject({ stage: 'uploading', uploadId: null, error: null });
		expect(sent('mod_upload_begin')).toHaveLength(2);
	});

	it("never streams into a cancelled upload whose begin answers after the next one's went out", async () => {
		await state.startUpload('client-a', file(), 'install');
		state.clearUpload();
		await state.startUpload('client-a', file(), 'install');
		expect(sent('mod_upload_begin')).toHaveLength(2);

		await modUploadBeginHandler.handle(
			{ upload_id: 'stale', chunk_size: 4, name: 'Cool Mod.zip' },
			context
		);
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(sent('mod_upload_chunk')).toEqual([]);
		expect(state.uploadStatus).toMatchObject({ stage: 'uploading', uploadId: null });

		await modUploadBeginHandler.handle(
			{ upload_id: 'fresh', chunk_size: 4, name: 'Cool Mod.zip' },
			context
		);
		await vi.waitFor(() => expect(sent('mod_upload_chunk')).toHaveLength(1));
		expect(sent('mod_upload_chunk')[0]).toMatchObject({ upload_id: 'fresh', seq: 0 });
	});

	it('sends no second chunk for a repeated begin reply', async () => {
		await begun();
		await modUploadBeginHandler.handle(
			{ upload_id: 'u1', chunk_size: 4, name: 'Cool Mod.zip' },
			context
		);
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(sent('mod_upload_chunk')).toHaveLength(1);
	});

	it('counts as abandoned only a begin that was sent and is still unanswered', async () => {
		const hashing = state.startUpload('client-a', file(), 'install');
		state.clearUpload();
		await hashing;
		await begun();
		state.clearUpload();

		await state.startUpload('client-a', file(), 'install');
		await modUploadBeginHandler.handle(
			{ upload_id: 'u2', chunk_size: 4, name: 'Cool Mod.zip' },
			context
		);
		await vi.waitFor(() =>
			expect(sent('mod_upload_chunk').at(-1)).toMatchObject({ upload_id: 'u2', seq: 0 })
		);
	});

	it('forgets abandoned begins when the connection drops, since their replies never come', async () => {
		state.connectionChanged(true);
		await state.startUpload('client-a', file(), 'install');
		state.clearUpload();
		state.connectionChanged(false);
		state.connectionChanged(true);

		await state.startUpload('client-a', file(), 'install');
		await modUploadBeginHandler.handle(
			{ upload_id: 'u2', chunk_size: 4, name: 'Cool Mod.zip' },
			context
		);
		await vi.waitFor(() => expect(sent('mod_upload_chunk')).toHaveLength(1));
		expect(sent('mod_upload_chunk')[0]).toMatchObject({ upload_id: 'u2', seq: 0 });
	});
});
