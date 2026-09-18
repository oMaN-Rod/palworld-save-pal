// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/utils/websocketUtils', () => ({ send: vi.fn(), sendAndWait: vi.fn() }));

import { NexusState } from '../nexusState.svelte';
import NexusHasMoreHarness from './NexusHasMoreHarness.svelte';

function summary(modId: number) {
	return {
		mod_id: modId,
		name: `Mod ${modId}`,
		summary: null,
		version: null,
		author: null,
		uploader: null,
		picture_url: null,
		thumbnail_url: null,
		endorsements: 0,
		downloads: 0,
		file_size: null,
		adult_content: false,
		created_at: null,
		updated_at: null,
		category: null
	};
}

/** `NexusState.hasMore` is a getter over a class field: only a real reactive read (a live
 *  component, not a direct property access) can prove the field backing it is `$state`. */
describe('NexusState.hasMore reactivity', () => {
	it('is false before any page, true after a page that leaves more, false once every result is loaded', async () => {
		const state = new NexusState();
		render(NexusHasMoreHarness, { state });

		expect(screen.getByTestId('has-more').textContent).toBe('false');

		state.search();
		state.finishBusy('searching');
		state.applySearch({ offset: 0, count: 2, total_count: 5, mods: [summary(1), summary(2)] });
		await tick();
		expect(screen.getByTestId('has-more').textContent).toBe('true');

		state.loadMore();
		state.applySearch({ offset: 2, count: 3, total_count: 5, mods: [summary(3)] });
		await tick();
		expect(screen.getByTestId('has-more').textContent).toBe('false');
	});
});
