// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';

import ResizeWarning from '../ResizeWarning.svelte';

function sizeWindow(width: number, height: number): void {
	Object.defineProperty(window, 'innerWidth', { value: width, configurable: true });
	Object.defineProperty(window, 'innerHeight', { value: height, configurable: true });
}

describe('ResizeWarning', () => {
	it('covers a window smaller than the app can lay out in', async () => {
		sizeWindow(360, 280);
		render(ResizeWarning);
		await tick();

		expect(screen.queryByRole('alert')).not.toBeNull();
	});

	it('leaves a window that is big enough alone', async () => {
		sizeWindow(1366, 768);
		render(ResizeWarning);
		await tick();

		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('stays out of an exempt window however small it is', async () => {
		sizeWindow(360, 280);
		render(ResizeWarning, { exempt: true });
		await tick();

		expect(screen.queryByRole('alert')).toBeNull();
	});
});
