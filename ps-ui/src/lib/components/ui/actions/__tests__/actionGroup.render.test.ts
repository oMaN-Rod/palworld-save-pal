// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

import type { ActionDescriptor } from '../actionDescriptor';

const { setViewport } = installViewportStub();
const { default: ActionGroup } = await import('../ActionGroup.svelte');

const actions: ActionDescriptor[] = [
	{ id: 'sort', label: 'Sort inventory', icon: 'tabler:sort-ascending', run: vi.fn() },
	{ id: 'fill', label: 'Fill inventory', icon: 'tabler:paint', run: vi.fn() }
];

describe('ActionGroup', () => {
	beforeEach(() => vi.clearAllMocks());

	it('renders a rail of labelled buttons on desktop', async () => {
		setViewport(1440);
		render(ActionGroup, { actions, title: 'Quick actions' });
		await tick();

		expect(screen.getByRole('button', { name: /sort inventory/i })).not.toBeNull();
		expect(screen.getByRole('button', { name: /fill inventory/i })).not.toBeNull();
		expect(screen.queryByRole('button', { name: /quick actions/i })).toBeNull();
	});

	it('renders a single trigger instead of a rail on phone', async () => {
		setViewport(390);
		render(ActionGroup, { actions, title: 'Quick actions' });
		await tick();

		expect(screen.getByRole('button', { name: /quick actions/i })).not.toBeNull();
		expect(screen.queryByRole('button', { name: /sort inventory/i })).toBeNull();
	});

	it('opens the action sheet from the phone trigger', async () => {
		setViewport(390);
		render(ActionGroup, { actions, title: 'Quick actions' });
		await tick();

		await userEvent.click(screen.getByRole('button', { name: /quick actions/i }));
		await tick();

		expect(screen.getByRole('dialog')).not.toBeNull();
		expect(screen.getByRole('button', { name: /sort inventory/i })).not.toBeNull();
	});

	it('closes the sheet when the device leaves phone', async () => {
		setViewport(390);
		render(ActionGroup, { actions, title: 'Quick actions' });
		await tick();

		await userEvent.click(screen.getByRole('button', { name: /quick actions/i }));
		await tick();
		expect(screen.getByRole('dialog')).not.toBeNull();

		setViewport(1440);
		await tick();

		setViewport(390);
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('runs the same descriptor from either presentation', async () => {
		setViewport(1440);
		render(ActionGroup, { actions, title: 'Quick actions' });
		await tick();

		await userEvent.click(screen.getByRole('button', { name: /sort inventory/i }));

		expect(actions[0].run).toHaveBeenCalledTimes(1);
	});
});
