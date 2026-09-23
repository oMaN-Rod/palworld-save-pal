// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { appState } = vi.hoisted(() => ({
	appState: {
		saveFile: { name: 'save' } as unknown,
		playerSummariesArray: [] as unknown[],
		guildSummariesArray: [] as unknown[]
	}
}));

vi.mock('$states', () => ({ getAppState: () => appState }));

vi.mock('$lib/components/bulk/PlayersTable.svelte', async () => ({
	default: (await import('./fixtures/BulkTableStub.svelte')).default
}));
vi.mock('$lib/components/bulk/PalsTable.svelte', async () => ({
	default: (await import('./fixtures/BulkTableStub.svelte')).default
}));
vi.mock('$lib/components/bulk/GuildsTable.svelte', async () => ({
	default: (await import('./fixtures/BulkTableStub.svelte')).default
}));

import RegistryPage from '../+page.svelte';

beforeEach(() => {
	vi.clearAllMocks();
	appState.saveFile = { name: 'save' };
});

describe('registry page', () => {
	it('offers a tab per entity', async () => {
		render(RegistryPage);
		await tick();

		expect(screen.getAllByRole('tab').length).toBe(3);
	});

	it('mounts only the table it is showing', async () => {
		render(RegistryPage);
		await tick();

		expect(screen.getAllByRole('tabpanel').length).toBe(1);
		expect(screen.getAllByTestId('bulk-table').length).toBe(1);
	});

	it('swaps the panel when another tab is picked', async () => {
		render(RegistryPage);
		await tick();

		const before = screen.getByRole('tabpanel').id;
		await fireEvent.click(screen.getByRole('tab', { name: /pals/i }));
		await tick();

		const after = screen.getByRole('tabpanel');
		expect(after.id).not.toBe(before);
		expect(screen.getAllByTestId('bulk-table').length).toBe(1);
	});

	it('names the panel after the tab that opened it', async () => {
		render(RegistryPage);
		await tick();

		const tab = screen
			.getAllByRole('tab')
			.find((el) => el.getAttribute('aria-selected') === 'true');
		const panel = screen.getByRole('tabpanel');
		expect(panel.getAttribute('aria-labelledby')).toBe(tab?.id);
		expect(tab?.getAttribute('aria-controls')).toBe(panel.id);
	});

	it('says so when no save is loaded', async () => {
		appState.saveFile = undefined;
		render(RegistryPage);
		await tick();

		expect(screen.queryByRole('tablist')).toBeNull();
	});
});
