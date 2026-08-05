import { describe, it, expect, beforeEach, vi } from 'vitest';

const nav = vi.hoisted(() => ({ goto: vi.fn(async () => {}) }));
vi.mock('$app/navigation', () => ({ goto: nav.goto }));
const ws = vi.hoisted(() => ({ send: vi.fn(), push: vi.fn() }));
vi.mock('$lib/utils/websocketUtils', () => ({ send: ws.send, pushProgressMessage: ws.push }));
const fs = vi.hoisted(() => ({ record: vi.fn(async () => ({ persisted: true, quota: false })) }));
vi.mock('$lib/fs', () => ({ recordSession: fs.record }));
const st = vi.hoisted(() => ({ reset: vi.fn(), toastAdd: vi.fn() }));
vi.mock('$states', () => ({
	getAppState: () => ({ resetState: st.reset }),
	getToastState: () => ({ add: st.toastAdd })
}));
vi.mock('$types', () => ({ MessageType: { LOAD_ZIP_FILE: 'load_zip_file' } }));

import { startSaveLoad } from './loadSave';

beforeEach(() => {
	vi.clearAllMocks();
	fs.record.mockResolvedValue({ persisted: true, quota: false });
});

describe('startSaveLoad', () => {
	it('navigates, resets, sends the zip, and records the session', async () => {
		const zip = new Uint8Array([1, 2, 3]);
		await startSaveLoad(zip, 'world1', { handle: {} as FileSystemDirectoryHandle, writable: true });
		expect(nav.goto).toHaveBeenCalledWith('/loading');
		expect(st.reset).toHaveBeenCalled();
		expect(ws.send).toHaveBeenCalledWith('load_zip_file', [1, 2, 3]);
		expect(fs.record).toHaveBeenCalledWith(
			expect.objectContaining({ name: 'world1', writable: true })
		);
		expect(st.toastAdd).not.toHaveBeenCalled();
	});

	it('toasts a warning when persistence hit the quota', async () => {
		fs.record.mockResolvedValue({ persisted: false, quota: true });
		await startSaveLoad(new Uint8Array([9]), 'big');
		expect(st.toastAdd).toHaveBeenCalledTimes(1);
	});
});
