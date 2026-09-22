// @vitest-environment jsdom
import { EntryState, PalGender, type Pal, type WorkSuitability } from '$types';
import '$utils/__tests__/fixtures/animatePolyfill';
import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';
import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { renderers } = vi.hoisted(() => ({ renderers: [] as { disposed: number }[] }));

// Only the GL context is faked; whether one gets built is under test.
vi.mock('three', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	class FakeRenderer {
		record = { disposed: 0 };
		constructor() {
			renderers.push(this.record);
		}
		dispose() {
			this.record.disposed += 1;
		}
		forceContextLoss() {}
		setClearAlpha() {}
		setPixelRatio() {}
		setSize() {}
		render() {}
	}
	return { ...actual, WebGLRenderer: FakeRenderer };
});

vi.mock('$components/map/scene/pal/palMeshLibrary', () => ({
	palModelUrl: (key: string) => `/models/pals/${key}.glb`,
	requestPalMesh: () => null,
	palMeshFailed: () => false,
	onPalMeshLoaded: () => () => {}
}));

const { appState } = vi.hoisted(() => ({
	appState: {
		selectedPal: undefined as Pal | undefined,
		selectedPlayer: undefined,
		settings: { language: 'en', cheat_mode: false }
	}
}));

vi.mock('$states', async (original) => ({
	...(await original<Record<string, unknown>>()),
	getAppState: () => appState
}));

const { setViewport } = installViewportStub();
const { default: PalEditModal } = await import('../PalEditModal.svelte');
const { expData, friendshipData } = await import('$lib/data');

const DESKTOP_WIDTH = 1920;
const TABLET_WIDTH = 820;
const PHONE_WIDTH = 390;

const ACTIVE_SKILL_MARKER = 'ZzActiveSkillMarkerZz';
const PASSIVE_SKILL_MARKER = 'ZzPassiveSkillMarkerZz';

function seedExp(): void {
	const rows: Record<string, unknown> = {};
	for (let level = 1; level <= 60; level++) {
		rows[String(level)] = {
			DropEXP: 0,
			NextEXP: 100,
			PalNextEXP: 100,
			TotalEXP: level * 100,
			PalTotalEXP: level * 100,
			BuildEXP: 0,
			CraftEXP: 0,
			PalBuildEXP: 0,
			PalCraftEXP: 0
		};
	}
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	expData.expData = rows as any;
}

function makePal(overrides: Partial<Pal> = {}): Pal {
	return {
		name: 'Testpal',
		instance_id: 'pal-1',
		character_id: 'TestPal',
		character_key: 'testpal',
		is_lucky: false,
		is_boss: false,
		is_predator: false,
		is_awakened: false,
		is_imported: false,
		friendship_point: 0,
		gender: PalGender.MALE,
		rank_hp: 0,
		rank_attack: 0,
		rank_defense: 0,
		rank_craftspeed: 0,
		talent_hp: 50,
		talent_shot: 50,
		talent_defense: 50,
		rank: 1,
		level: 15,
		is_tower: false,
		stomach: 100,
		storage_slot: 0,
		learned_skills: [],
		active_skills: [ACTIVE_SKILL_MARKER],
		passive_skills: [PASSIVE_SKILL_MARKER],
		work_suitability: {} as Record<WorkSuitability, number>,
		hp: 100,
		max_hp: 100,
		elements: [],
		state: EntryState.NONE,
		sanity: 100,
		exp: 0,
		is_sick: false,
		...overrides
	};
}

type Section = { label: string; matches: () => HTMLElement[]; trigger: RegExp };

function sections(): Section[] {
	return [
		{
			label: 'active skills',
			matches: () => screen.queryAllByText(ACTIVE_SKILL_MARKER),
			trigger: /active skills/i
		},
		{
			label: 'passive skills',
			matches: () => screen.queryAllByText(PASSIVE_SKILL_MARKER),
			trigger: /passive skills/i
		},
		{
			label: 'work suitability',
			matches: () => screen.queryAllByAltText('EmitFlame icon'),
			trigger: /work suitability/i
		},
		{
			label: 'talents',
			matches: () => screen.queryAllByAltText('HP icon'),
			trigger: /talents/i
		},
		{
			label: 'souls',
			matches: () => screen.queryAllByAltText('Workspeed icon'),
			trigger: /souls/i
		}
	];
}

// A default-open accordion panel can duplicate the column copy; one toggle of its trigger removes it.
async function assertBodyExactlyOnce(section: Section) {
	let matches = section.matches();
	if (matches.length !== 1) {
		const trigger = screen.queryByRole('button', { name: section.trigger });
		if (trigger) {
			await userEvent.click(trigger);
			await tick();
			matches = section.matches();
		}
	}
	expect(matches, section.label).toHaveLength(1);
}

describe('PalEditModal sections', () => {
	beforeEach(() => {
		seedExp();
		friendshipData.friendshipData = { '1': { rank: 1, required_point: 0 } };
		appState.selectedPal = makePal();
		appState.selectedPlayer = undefined;
		renderers.length = 0;
		vi.stubGlobal(
			'ResizeObserver',
			class {
				observe() {}
				disconnect() {}
			}
		);
	});

	it('renders all five sections with content present at desktop width', async () => {
		setViewport(DESKTOP_WIDTH);
		render(PalEditModal);
		await tick();

		for (const section of sections()) {
			expect(section.matches().length, section.label).toBeGreaterThan(0);
		}
	});

	it('renders the same five sections as collapsibles at tablet width', async () => {
		setViewport(TABLET_WIDTH);
		render(PalEditModal);
		await tick();

		for (const section of sections()) {
			expect(screen.getByRole('button', { name: section.trigger }), section.label).not.toBeNull();
		}
	});

	it('renders each section body exactly once at desktop width', async () => {
		setViewport(DESKTOP_WIDTH);
		render(PalEditModal);
		await tick();

		for (const section of sections()) {
			await assertBodyExactlyOnce(section);
		}
	});

	it('renders each section body exactly once at tablet width', async () => {
		setViewport(TABLET_WIDTH);
		render(PalEditModal);
		await tick();

		for (const section of sections()) {
			await assertBodyExactlyOnce(section);
		}
	});

	it('keeps talents and souls in a separate desktop column from the other three', async () => {
		setViewport(DESKTOP_WIDTH);
		render(PalEditModal);
		await tick();

		const primaryColumn = screen.getByTestId('active_skills').parentElement;
		expect(screen.getByTestId('passive_skills').parentElement).toBe(primaryColumn);
		expect(screen.getByTestId('work_suitability').parentElement).toBe(primaryColumn);

		const asideColumn = screen.getByTestId('talents').parentElement;
		expect(screen.getByTestId('souls').parentElement).toBe(asideColumn);
		expect(asideColumn).not.toBe(primaryColumn);
	});

	it('keeps the identity strip and a tablist on a phone', async () => {
		setViewport(PHONE_WIDTH);
		render(PalEditModal);
		await tick();

		expect(document.getElementById('pal-identity')).not.toBeNull();
		expect(document.getElementById('pal-header')).not.toBeNull();
		expect(screen.getByRole('tablist', { name: 'Pal sections' })).not.toBeNull();
	});

	it('gives status and stats a tab of their own on a phone', async () => {
		setViewport(PHONE_WIDTH);
		render(PalEditModal);
		await tick();

		const list = screen.getByRole('tablist', { name: 'Pal sections' });
		const labels = within(list)
			.getAllByRole('tab')
			.map((tab) => tab.textContent?.trim());
		expect(labels).toContain('Stats');
	});

	it('offers the 3D model rather than building a GL context for it', async () => {
		setViewport(PHONE_WIDTH);
		render(PalEditModal);
		await tick();

		expect(screen.getByRole('button', { name: 'Load 3D model' })).not.toBeNull();
		expect(document.querySelector('canvas')).toBeNull();
		expect(renderers).toHaveLength(0);
	});

	it('builds the model only once the user asks for it', async () => {
		setViewport(PHONE_WIDTH);
		render(PalEditModal);
		await tick();

		await userEvent.click(screen.getByRole('button', { name: 'Load 3D model' }));
		await tick();

		expect(document.querySelector('canvas')).not.toBeNull();
		expect(renderers).toHaveLength(1);
	});

	it('places the pal portrait between the primary and aside desktop columns', async () => {
		setViewport(DESKTOP_WIDTH);
		render(PalEditModal);
		await tick();

		const primary = screen.getByTestId('active_skills');
		const portrait = document.getElementById('pal-image');
		const aside = screen.getByTestId('talents');

		expect(portrait).not.toBeNull();
		expect(
			primary.compareDocumentPosition(portrait!) & Node.DOCUMENT_POSITION_FOLLOWING
		).toBeTruthy();
		expect(
			portrait!.compareDocumentPosition(aside) & Node.DOCUMENT_POSITION_FOLLOWING
		).toBeTruthy();
	});
});
