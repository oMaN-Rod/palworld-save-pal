// @vitest-environment jsdom
import type { Server } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder } = vi.hoisted(() => ({ holder: { server: undefined as unknown } }));

vi.mock('$lib/utils/websocketUtils', () => ({ send: vi.fn(), sendAndWait: vi.fn() }));

vi.mock('$states', async () => {
	const { ServerState } = await import('$lib/states/serverState.svelte');
	holder.server = new ServerState();
	return { getServerState: () => holder.server };
});

import { ServerState } from '$lib/states/serverState.svelte';
import ServerCard from '../ServerCard.svelte';

const state = () => holder.server as ServerState;

function server(overrides: Partial<Server> = {}): Server {
	return {
		id: 7,
		name: 'Main World',
		server_type: 'docker',
		container_name: 'psp-main',
		game_port: 8211,
		max_players: 16,
		status: { status: 'exited', running: false },
		...overrides
	} as Server;
}

function renderCard(value: Server) {
	const handlers = { onselect: vi.fn(), onstart: vi.fn(), onstop: vi.fn() };
	const { container } = render(ServerCard, { server: value, ...handlers });
	const toggle = container.querySelector('[data-server-toggle]') as HTMLButtonElement;
	return { container, handlers, toggle };
}

beforeEach(() => {
	holder.server = new ServerState();
});

describe('ServerCard start button', () => {
	it('starts a stopped server', async () => {
		const { handlers, toggle } = renderCard(server());

		expect(toggle.disabled).toBe(false);
		await fireEvent.click(toggle);

		expect(handlers.onstart).toHaveBeenCalledTimes(1);
		expect(handlers.onselect).not.toHaveBeenCalled();
	});

	it('is busy while the server is starting', async () => {
		const { handlers, toggle } = renderCard(server());

		state().starting = { 7: true };
		await tick();

		expect(toggle.disabled).toBe(true);
		expect(toggle.getAttribute('aria-busy')).toBe('true');
		await fireEvent.click(toggle);
		expect(handlers.onstart).not.toHaveBeenCalled();

		state().starting = {};
		await tick();
		expect(toggle.disabled).toBe(false);
	});

	it('keeps Stop available for a running server', () => {
		state().starting = { 7: true };
		const { toggle } = renderCard(server({ status: { status: 'running', running: true } }));

		expect(toggle.disabled).toBe(false);
		expect(toggle.getAttribute('aria-busy')).toBe('false');
	});
});

describe('ServerCard selection', () => {
	it('selects the server without nesting the start button in another button', async () => {
		const { handlers, toggle } = renderCard(server());

		expect(toggle.parentElement?.closest('button')).toBeNull();
		await fireEvent.click(screen.getByRole('button', { name: 'Main World' }));

		expect(handlers.onselect).toHaveBeenCalledTimes(1);
		expect(handlers.onstart).not.toHaveBeenCalled();
	});

	it('lets pointer clicks pass through the card to the select button, except on its controls', () => {
		const { toggle } = renderCard(server());
		const select = screen.getByRole('button', { name: 'Main World' });
		// The card body is the select button's sibling, laid over it, so a click
		// anywhere that is not a control falls through to the button beneath.
		const card = select.nextElementSibling as HTMLElement;

		expect(card).toBeTruthy();
		expect(card.contains(select)).toBe(false);
		expect(card.classList.contains('pointer-events-none')).toBe(true);
		const controls = card.querySelectorAll('button, a, input, select, textarea');
		expect(controls).toHaveLength(1);
		expect(controls[0]).toBe(toggle);
		expect(toggle.classList.contains('pointer-events-auto')).toBe(true);
	});
});

describe('ServerCard names', () => {
	it('names the start toggle of a stopped server', () => {
		renderCard(server());
		expect(screen.getByRole('button', { name: 'Start server' })).toBeTruthy();
	});

	it('names the stop toggle of a running server', () => {
		renderCard(server({ status: { status: 'running', running: true } }));
		expect(screen.getByRole('button', { name: 'Stop server' })).toBeTruthy();
	});
});
