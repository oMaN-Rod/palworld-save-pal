// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { modalState, toastState } = vi.hoisted(() => ({
	modalState: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	toastState: { add: vi.fn() }
}));

const PRESET_PROFILES: Record<string, { name: string; type: string; pal_preset?: unknown }> = {
	'preset-a': { name: 'Alpha Loadout', type: 'pal_preset', pal_preset: {} },
	'preset-b': { name: 'Bravo Loadout', type: 'pal_preset', pal_preset: {} }
};

vi.mock('$lib/data', () => ({
	presetsData: { presetProfiles: PRESET_PROFILES }
}));

vi.mock('$states', () => ({
	getModalState: () => modalState,
	getToastState: () => toastState,
	getConfig: () => ({ mode: 'name', direction: 'asc' }),
	setMode: vi.fn(),
	setDirection: vi.fn(),
	setCustomOrder: vi.fn(),
	sortPresets: (presets: { name: string }[]) =>
		[...presets].sort((a, b) => a.name.localeCompare(b.name))
}));

vi.mock('$utils/websocketUtils', () => ({ sendAndWait: vi.fn() }));

// The detail panels need game data this page does not own.
vi.mock('../components/PalPreset.svelte', () => ({ default: vi.fn() }));
vi.mock('../components/ActiveSkills.svelte', () => ({ default: vi.fn() }));
vi.mock('../components/PassiveSkills.svelte', () => ({ default: vi.fn() }));
vi.mock('../components/PlayerInventory.svelte', () => ({ default: vi.fn() }));
vi.mock('../components/StorageInventory.svelte', () => ({ default: vi.fn() }));

// Installed before the page import so the layout store reads the stubbed `matchMedia`.
const { setViewport } = installViewportStub();
const { default: PresetsPage } = await import('../+page.svelte');

function selectFirstPreset() {
	const row = screen.getByText('Alpha Loadout').closest('button');
	expect(row).not.toBeNull();
	return fireEvent.click(row!);
}

beforeEach(() => {
	vi.clearAllMocks();
	setViewport(1440);
});

describe('presets page', () => {
	it('shows the detail beside the list on a desktop', async () => {
		render(PresetsPage);
		await tick();
		await selectFirstPreset();
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(document.querySelector('#presets-detail')).not.toBeNull();
	});

	it('lifts the detail into a sheet on a phone', async () => {
		setViewport(390);
		render(PresetsPage);
		await tick();
		await selectFirstPreset();
		await tick();

		const sheet = screen.getByRole('dialog');
		expect(within(sheet).getByTestId('presets-detail')).not.toBeNull();
	});

	it('leaves the list at full width until a preset is picked', async () => {
		setViewport(390);
		render(PresetsPage);
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(screen.getByText('Alpha Loadout')).not.toBeNull();
	});

	it('clears the selection when the sheet is dismissed', async () => {
		setViewport(390);
		render(PresetsPage);
		await tick();
		await selectFirstPreset();
		await tick();

		await fireEvent.click(screen.getByTestId('sheet-backdrop'));
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
	});
});
