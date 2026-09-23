// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { appState, palEditor, modalState } = vi.hoisted(() => ({
	appState: {
		selectedPlayer: undefined as unknown,
		guilds: {} as Record<string, unknown>,
		loadingGuild: false,
		clipboardItem: null as unknown,
		settings: { debug_mode: false, new_pal_prefix: '', clone_prefix: '' }
	},
	palEditor: { open: vi.fn() },
	modalState: { showModal: vi.fn(), showConfirmModal: vi.fn() }
}));

vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => modalState,
	getToastState: () => ({ add: vi.fn() }),
	getPalEditorState: () => palEditor
}));

vi.mock('$lib/utils/websocketUtils', () => ({ send: vi.fn() }));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

vi.mock('$components/modals', () => ({
	PalSelectModal: {},
	NumberInputModal: {},
	PalPresetSelectModal: {},
	NumberSliderModal: {},
	TextInputModal: {}
}));

vi.mock('$components/shared', () => ({ ItemBadge: {} }));
vi.mock('$components/presets', () => ({ StoragePresets: {} }));
vi.mock('$components/guilds', () => ({ LabResearchControls: {} }));
vi.mock('$components/guilds/LabResearch.svelte', () => ({ default: {} }));

vi.mock('$lib/data', () => ({
	palsData: { getByKey: () => undefined },
	itemsData: { getByKey: () => undefined },
	presetsData: { presetProfiles: {} }
}));

// The stub must be installed before the page pulls in the layout store.
const { setViewport } = installViewportStub();
const { default: GuildPage } = await import('../+page.svelte');

function loadGuild() {
	appState.selectedPlayer = { uid: 'p1', guild_id: 'g1', level: 10 };
	appState.guilds = {
		g1: {
			id: 'g1',
			name: 'Test Guild',
			base_camp_level: 3,
			bases: {
				b1: {
					id: 'b1',
					name: 'Base One',
					container_id: 'c1',
					slot_count: 2,
					pals: {},
					storage_containers: {}
				}
			}
		}
	};
}

beforeEach(() => {
	vi.clearAllMocks();
	setViewport(390);
	loadGuild();
});

// jsdom applies no CSS, so these pin the breakpoint classes instead.
describe('guild page layout', () => {
	it('stacks the sidebar over the content below the breakpoint', async () => {
		render(GuildPage);
		await tick();

		const shell = document.querySelector('#guild-shell');
		expect(shell).not.toBeNull();
		expect(shell?.className).toContain('flex-col');
		expect(shell?.className).toContain('md:grid');
		const unprefixed = (shell?.className ?? '')
			.split(/  */)
			.filter((token) => !token.includes(':'));
		expect(unprefixed.some((token) => token.startsWith('grid-cols-'))).toBe(false);
	});

	it('keeps the four tabs on one row', async () => {
		render(GuildPage);
		await tick();

		const tabs = document.querySelector('#guild-tabs');
		expect(tabs).not.toBeNull();
		expect(tabs?.className).not.toContain('flex-col');
		expect(tabs?.className).toContain('overflow-x-auto');
	});

	it('sizes each tab by the row rather than by the viewport', async () => {
		render(GuildPage);
		await tick();

		const buttons = document.querySelectorAll('#guild-tabs button');
		expect(buttons.length).toBe(4);
		for (const button of buttons) {
			expect(button.className).not.toContain('w-1/4');
			expect(button.className).toContain('flex-1');
		}
	});

	it('gives every tab a thumb-sized target', async () => {
		render(GuildPage);
		await tick();

		const buttons = document.querySelectorAll('#guild-tabs button');
		for (const button of buttons) {
			expect(button.className).toContain('min-h-11');
		}
	});

	it('still reaches every section', async () => {
		render(GuildPage);
		await tick();

		for (const id of ['guild-tab-pals', 'guild-tab-storage', 'guild-tab-chest', 'guild-tab-lab']) {
			expect(document.querySelector(`#${id}`)).not.toBeNull();
		}
		expect(screen.queryByText('Test Guild')).not.toBeNull();
	});
});
