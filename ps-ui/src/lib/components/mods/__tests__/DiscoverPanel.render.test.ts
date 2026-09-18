// @vitest-environment jsdom
import type { NexusModSummary } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, send, modal } = vi.hoisted(() => ({
	holder: { mods: undefined as unknown, nexus: undefined as unknown },
	send: vi.fn(),
	modal: { showModal: vi.fn() }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	const { NexusState } = await import('$lib/states/nexusState.svelte');
	holder.mods = new ModsState();
	holder.nexus = new NexusState();
	return {
		getModsState: () => holder.mods,
		getNexusState: () => holder.nexus,
		getModalState: () => modal,
		getToastState: () => ({ add: vi.fn() })
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import type { NexusState } from '$lib/states/nexusState.svelte';
import { nexusSearchHandler } from '$lib/ws/handlers/nexusHandler';
import DiscoverPanel from '../DiscoverPanel.svelte';
import { libraryMod } from './fixtures';

const context = {} as never;
const modsState = () => holder.mods as ModsState;
const nexusState = () => holder.nexus as NexusState;

function summary(overrides: Partial<NexusModSummary> = {}): NexusModSummary {
	return {
		mod_id: 4821,
		name: 'Cool Mod',
		summary: null,
		version: null,
		author: null,
		uploader: null,
		picture_url: null,
		thumbnail_url: null,
		endorsements: 3,
		downloads: 10,
		file_size: null,
		adult_content: false,
		created_at: null,
		updated_at: null,
		category: null,
		...overrides
	};
}

function sent(type: MessageType) {
	return send.mock.calls.filter((call) => call[0] === type);
}

beforeEach(() => {
	send.mockReset();
	modal.showModal.mockReset();
	modsState().reset();
	nexusState().reset();
});

describe('DiscoverPanel', () => {
	it('runs a first browse and loads categories on mount', () => {
		render(DiscoverPanel, { targetId: 'client-abc' });

		expect(sent(MessageType.NEXUS_SEARCH)).toHaveLength(1);
		expect(sent(MessageType.NEXUS_CATEGORIES)).toHaveLength(1);
	});

	it('does not send a request while typing, only after the debounce settles', async () => {
		render(DiscoverPanel, { targetId: 'client-abc' });
		send.mockClear();
		vi.useFakeTimers();

		await fireEvent.input(screen.getByLabelText('Search Nexus Mods'), {
			target: { value: 'bag' }
		});
		expect(sent(MessageType.NEXUS_SEARCH)).toHaveLength(0);

		vi.advanceTimersByTime(300);
		expect(sent(MessageType.NEXUS_SEARCH)).toHaveLength(1);
		expect(nexusState().query).toBe('bag');

		vi.useRealTimers();
	});

	it('renders one card per mod result, each with a details button named after it', async () => {
		render(DiscoverPanel, { targetId: 'client-abc' });
		await nexusSearchHandler.handle(
			{
				offset: 0,
				count: 2,
				total_count: 2,
				mods: [summary({ mod_id: 1, name: 'Alpha' }), summary({ mod_id: 2, name: 'Bravo' })]
			},
			context
		);
		await tick();

		expect(screen.getByRole('button', { name: 'Details for Alpha' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Details for Bravo' })).toBeTruthy();
	});

	it('shows the library badge only on a mod already in the library', async () => {
		modsState().mods = [libraryMod({ id: 'mod-a', nexus_mod_id: 4821 })];
		render(DiscoverPanel, { targetId: 'client-abc' });
		await nexusSearchHandler.handle(
			{
				offset: 0,
				count: 2,
				total_count: 2,
				mods: [summary({ mod_id: 4821, name: 'Cool Mod' }), summary({ mod_id: 55, name: 'Other' })]
			},
			context
		);
		await tick();

		const installedCard = document.querySelector('[data-nexus-mod-id="4821"]') as HTMLElement;
		const otherCard = document.querySelector('[data-nexus-mod-id="55"]') as HTMLElement;
		expect(within(installedCard).getByText('In your library')).toBeTruthy();
		expect(within(otherCard).queryByText('In your library')).toBeNull();
	});

	it('shows Load more only while more results exist, sends the next offset, and disables while searching', async () => {
		render(DiscoverPanel, { targetId: 'client-abc' });
		await nexusSearchHandler.handle(
			{ offset: 0, count: 1, total_count: 2, mods: [summary({ mod_id: 1 })] },
			context
		);
		await tick();

		const loadMore = screen.getByRole('button', { name: 'Load more' }) as HTMLButtonElement;
		send.mockClear();
		await fireEvent.click(loadMore);

		expect(sent(MessageType.NEXUS_SEARCH)[0][1]).toMatchObject({ offset: 1 });
		expect(loadMore.disabled).toBe(true);

		await nexusSearchHandler.handle(
			{ offset: 1, count: 1, total_count: 2, mods: [summary({ mod_id: 2 })] },
			context
		);
		await tick();

		expect(screen.queryByRole('button', { name: 'Load more' })).toBeNull();
	});

	it('renders a nexus_search refusal recorded on ModsState', async () => {
		render(DiscoverPanel, { targetId: 'client-abc' });
		await nexusSearchHandler.handle(
			{ offset: 0, error: { code: 'network', message: 'no route' } },
			context
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('Nexus Mods could not be reached.');
	});

	it('matches the shown sort default to what the store actually sends', () => {
		const sortCombo = () =>
			(screen.getByText('Sort').parentElement as HTMLElement).querySelector(
				'[role="combobox"]'
			) as HTMLElement;

		const blank = render(DiscoverPanel, { targetId: 'client-abc' });
		expect(sortCombo().textContent).toContain('Downloads');
		blank.unmount();

		nexusState().query = 'bag';
		render(DiscoverPanel, { targetId: 'client-abc' });
		expect(sortCombo().textContent).toContain('Relevance');
	});

	it('sends a search for sort: relevance when Relevance is chosen with a blank query', async () => {
		render(DiscoverPanel, { targetId: 'client-abc' });
		send.mockClear();
		const sortCombo = (screen.getByText('Sort').parentElement as HTMLElement).querySelector(
			'[role="combobox"]'
		) as HTMLElement;
		await fireEvent.click(sortCombo);
		await fireEvent.click(screen.getByRole('option', { name: 'Relevance' }));

		expect(sent(MessageType.NEXUS_SEARCH).at(-1)?.[1]).toMatchObject({ sort: 'relevance' });
	});

	it('renders the initials fallback and no img for a javascript: thumbnail_url — the XSS guard', async () => {
		render(DiscoverPanel, { targetId: 'client-abc' });
		await nexusSearchHandler.handle(
			{
				offset: 0,
				count: 1,
				total_count: 1,
				mods: [summary({ mod_id: 1, name: 'Evil Mod', thumbnail_url: 'javascript:alert(1)' })]
			},
			context
		);
		await tick();

		expect(screen.getByText('EM')).toBeTruthy();
		expect(document.querySelector('img')).toBeNull();
	});
});
