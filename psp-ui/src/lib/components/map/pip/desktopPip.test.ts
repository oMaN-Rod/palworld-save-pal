import { describe, expect, it, vi } from 'vitest';
import { openDesktopPip } from './desktopPip';

describe('openDesktopPip', () => {
	it('asks the desktop shell to open the pip window', async () => {
		const invoke = vi.fn().mockResolvedValue(undefined);
		const onError = vi.fn();

		await openDesktopPip(invoke, onError);

		expect(invoke).toHaveBeenCalledWith('open_pip');
		expect(onError).not.toHaveBeenCalled();
	});

	it('reports the reason the shell gave for refusing', async () => {
		const invoke = vi.fn().mockRejectedValue('no main window');
		const onError = vi.fn();

		await openDesktopPip(invoke, onError);

		expect(onError).toHaveBeenCalledWith('no main window');
	});

	it('unwraps an Error rather than reporting [object Object]', async () => {
		const invoke = vi.fn().mockRejectedValue(new Error('window builder failed'));
		const onError = vi.fn();

		await openDesktopPip(invoke, onError);

		expect(onError).toHaveBeenCalledWith('window builder failed');
	});

	it('does nothing when there is no desktop shell to ask', async () => {
		const onError = vi.fn();

		await expect(openDesktopPip(undefined, onError)).resolves.toBeUndefined();
		expect(onError).not.toHaveBeenCalled();
	});
});
