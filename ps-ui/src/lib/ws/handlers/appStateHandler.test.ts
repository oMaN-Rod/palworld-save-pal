import { describe, expect, it, vi, beforeEach } from 'vitest';
import { MessageType } from '$types';

const toasts: Array<{ message: string; title?: string; color?: string }> = [];

vi.mock('$states', () => ({
	getToastState: () => ({
		add: (message: string, title?: string, color?: string) => {
			toasts.push({ message, title, color });
		}
	}),
	getAppState: () => ({}),
	getModalState: () => ({})
}));

const { warningHandler } = await import('./appStateHandler');

beforeEach(() => {
	toasts.length = 0;
});

describe('warningHandler', () => {
	it('is registered for the warning message type', () => {
		expect(warningHandler.type).toBe(MessageType.WARNING);
	});

	it('surfaces the warning as a toast rather than navigating to the error page', async () => {
		const goto = vi.fn();
		await warningHandler.handle(
			{ message: 'This browser cannot store data.', trace: '' },
			{ goto }
		);

		expect(toasts).toHaveLength(1);
		expect(toasts[0].message).toBe('This browser cannot store data.');
		expect(toasts[0].color).toBe('warning');
		// The whole point of this handler: a non-fatal warning must not eject the
		// user out of a working app the way MessageType.ERROR does.
		expect(goto).not.toHaveBeenCalled();
	});

	it('accepts a bare string payload', async () => {
		const goto = vi.fn();
		await warningHandler.handle('Storage is unavailable.', { goto });

		expect(toasts).toHaveLength(1);
		expect(toasts[0].message).toBe('Storage is unavailable.');
		expect(goto).not.toHaveBeenCalled();
	});

	it('ignores a payload with no usable message instead of toasting an empty string', async () => {
		const goto = vi.fn();
		await warningHandler.handle({ trace: 'x' }, { goto });

		expect(toasts).toHaveLength(0);
		expect(goto).not.toHaveBeenCalled();
	});
});
