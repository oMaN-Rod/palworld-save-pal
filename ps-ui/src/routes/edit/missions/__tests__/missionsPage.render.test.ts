// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { appState, modalState, toastState } = vi.hoisted(() => ({
	appState: {
		selectedPlayer: undefined as unknown
	},
	modalState: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	toastState: { add: vi.fn() }
}));

vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => modalState,
	getToastState: () => toastState
}));

vi.mock('$components/modals', () => ({ ConfirmModal: {} }));

const MISSIONS = {
	Main_1: {
		id: 'Main_1',
		localized_name: 'First Steps',
		description: 'Collect wood.',
		quest_type: 'Main',
		rewards: {}
	},
	Main_2: {
		id: 'Main_2',
		localized_name: 'Second Wind',
		description: 'Build a base.',
		quest_type: 'Main',
		rewards: {}
	}
};

vi.mock('$lib/data', () => ({
	missionsData: { getByKey: (key: string) => MISSIONS[key as keyof typeof MISSIONS] }
}));

vi.mock('$utils', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	return {
		...actual,
		assetLoader: { loadImage: (path: string) => path },
		calculateFilters: () => ''
	};
});

// The stub must be installed before the page pulls in the layout store.
const { setViewport } = installViewportStub();
const { default: MissionsPage } = await import('../+page.svelte');

function selectFirstMission() {
	const row = screen.getByText('First Steps').closest('button');
	expect(row).not.toBeNull();
	return fireEvent.click(row!);
}

beforeEach(() => {
	vi.clearAllMocks();
	setViewport(1440);
	appState.selectedPlayer = {
		current_missions: ['Main_1'],
		completed_missions: ['Main_2'],
		state: 0
	};
});

describe('missions page', () => {
	it('shows the detail beside the list on a desktop', async () => {
		render(MissionsPage);
		await tick();
		await selectFirstMission();
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(screen.getByText('Collect wood.')).not.toBeNull();
	});

	it('lifts the detail into a sheet on a phone', async () => {
		setViewport(390);
		render(MissionsPage);
		await tick();
		await selectFirstMission();
		await tick();

		const sheet = screen.getByRole('dialog');
		expect(within(sheet).getByText('Collect wood.')).not.toBeNull();
	});

	it('leaves the list at full width until a mission is picked', async () => {
		setViewport(390);
		render(MissionsPage);
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(screen.getByText('First Steps')).not.toBeNull();
	});

	it('clears the selection when the sheet is dismissed', async () => {
		setViewport(390);
		render(MissionsPage);
		await tick();
		await selectFirstMission();
		await tick();

		// A modal sheet has no close button of its own; the backdrop is the target.
		await fireEvent.click(screen.getByTestId('sheet-backdrop'));
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('keeps the bulk actions out of the tab strip on a phone', async () => {
		setViewport(390);
		render(MissionsPage);
		await tick();

		const actions = document.querySelector('#missions-actions');
		expect(actions).not.toBeNull();
		expect(actions?.className).toContain('md:absolute');
		expect(actions?.className.split(/\s+/)).not.toContain('absolute');
	});
});
