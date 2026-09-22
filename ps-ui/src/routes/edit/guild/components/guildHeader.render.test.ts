// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import GuildHeader from './GuildHeader.svelte';

const guild = {
	id: 'guild-1',
	name: 'Wandering Merchants',
	base_camp_level: 14,
	admin_player_uid: '',
	bases: {},
	players: [],
	state: 0
} as never;

const base = { id: 'base-7', name: 'Outpost' } as never;

function renderHeader(props: Record<string, unknown> = {}) {
	const handlers = {
		onEditGuildName: vi.fn(),
		onEditBasecampLevel: vi.fn(),
		onDeleteGuild: vi.fn(),
		onEditBaseName: vi.fn()
	};
	render(GuildHeader, {
		props: { guild, base, baseNumber: 2, debugMode: false, ...handlers, ...props }
	});
	return handlers;
}

/** `DebugButton` renders an icon button and nothing else identifying. */
function debugButtons(): HTMLElement[] {
	return Array.from(document.querySelectorAll('button.btn-icon')) as HTMLElement[];
}

function text(id: string): string {
	return document.querySelector(id)?.textContent?.trim() ?? '';
}

describe('GuildHeader', () => {
	it('names the guild, its camp level and the base on screen', () => {
		renderHeader();
		expect(text('#guild-name')).toBe('Wandering Merchants');
		expect(text('#guild-level')).toContain('14');
		expect(text('#guild-base-name')).toBe('Outpost');
	});

	it('numbers a base with no name of its own', () => {
		renderHeader({ base: { id: 'base-7' } as never, baseNumber: 3 });
		expect(text('#guild-base-name')).toBe('');
		expect(screen.getByText('Base 3')).toBeTruthy();
	});

	it('asks the page to edit, never edits itself', async () => {
		const user = userEvent.setup();
		const handlers = renderHeader();

		await user.click(document.querySelector('#guild-name') as HTMLElement);
		await user.click(document.querySelector('#guild-level') as HTMLElement);
		await user.click(document.querySelector('#guild-base-name') as HTMLElement);
		await user.click(document.querySelector('#guild-delete') as HTMLElement);

		expect(handlers.onEditGuildName).toHaveBeenCalledTimes(1);
		expect(handlers.onEditBasecampLevel).toHaveBeenCalledTimes(1);
		expect(handlers.onEditBaseName).toHaveBeenCalledTimes(1);
		expect(handlers.onDeleteGuild).toHaveBeenCalledTimes(1);
	});

	it('keeps the debug buttons out of a normal session', () => {
		renderHeader({ debugMode: true });
		const withDebug = debugButtons().length;
		document.body.innerHTML = '';
		renderHeader();
		expect(debugButtons()).toHaveLength(0);
		expect(withDebug).toBe(2);
	});

	it('drops the base debug button when there is no base', () => {
		renderHeader({ base: null, debugMode: true });
		expect(debugButtons()).toHaveLength(1);
	});
});
