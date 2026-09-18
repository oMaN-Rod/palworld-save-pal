import { describe, expect, it, vi } from 'vitest';
import { createWindowControls } from '../windowControls.svelte';

function fakeWindow(maximized = false) {
	let resized: (() => void) | undefined;
	const unlisten = vi.fn();
	return {
		minimize: vi.fn().mockResolvedValue(undefined),
		toggleMaximize: vi.fn().mockResolvedValue(undefined),
		close: vi.fn().mockResolvedValue(undefined),
		isMaximized: vi.fn().mockImplementation(() => Promise.resolve(maximized)),
		onResized: vi.fn().mockImplementation((handler: () => void) => {
			resized = handler;
			return Promise.resolve(unlisten);
		}),
		fire: () => resized?.(),
		unlisten,
		setMaximized: (value: boolean) => (maximized = value)
	};
}

describe('createWindowControls', () => {
	it('delegates each command to the current window', () => {
		const win = fakeWindow();
		const controls = createWindowControls(() => win);

		controls.minimize();
		controls.toggleMaximize();
		controls.close();

		expect(win.minimize).toHaveBeenCalledOnce();
		expect(win.toggleMaximize).toHaveBeenCalledOnce();
		expect(win.close).toHaveBeenCalledOnce();
	});

	it('reads the maximized state on sync', async () => {
		const win = fakeWindow(true);
		const controls = createWindowControls(() => win);

		expect(controls.maximized).toBe(false);
		await controls.sync();

		expect(controls.maximized).toBe(true);
	});

	it('re-syncs when the window is resized', async () => {
		const win = fakeWindow(false);
		const controls = createWindowControls(() => win);
		controls.watch();
		await vi.waitFor(() => expect(win.onResized).toHaveBeenCalled());

		win.setMaximized(true);
		win.fire();

		await vi.waitFor(() => expect(controls.maximized).toBe(true));
	});

	it('unlistens on teardown', async () => {
		const win = fakeWindow();
		const controls = createWindowControls(() => win);
		const stop = controls.watch();
		await vi.waitFor(() => expect(win.onResized).toHaveBeenCalled());

		stop();

		expect(win.unlisten).toHaveBeenCalledOnce();
	});

	it('unlistens even when teardown runs before onResized resolves', async () => {
		const win = fakeWindow();
		const controls = createWindowControls(() => win);
		const stop = controls.watch();

		stop();
		await vi.waitFor(() => expect(win.unlisten).toHaveBeenCalledOnce());
	});

	it('is inert when there is no Tauri window', () => {
		const controls = createWindowControls(() => undefined);

		expect(() => {
			controls.minimize();
			controls.watch()();
		}).not.toThrow();
		expect(controls.maximized).toBe(false);
	});
});
