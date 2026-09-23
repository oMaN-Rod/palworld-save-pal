// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it } from 'vitest';

import { navDrawer } from '$states/navDrawer.svelte';
import AppBar from '../AppBar.svelte';

beforeEach(() => {
	navDrawer.open = false;
});

describe('AppBar', () => {
	it('exposes a drawer trigger', async () => {
		render(AppBar);
		await tick();

		expect(screen.getByRole('button', { name: /menu/i })).not.toBeNull();
	});

	it('opens the nav drawer when the trigger is activated', async () => {
		render(AppBar);
		await tick();

		await fireEvent.click(screen.getByRole('button', { name: /menu/i }));

		expect(navDrawer.open).toBe(true);
	});
});
