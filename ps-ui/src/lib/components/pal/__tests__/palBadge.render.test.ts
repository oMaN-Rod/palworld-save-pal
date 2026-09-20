// @vitest-environment jsdom
import { getAppState, getPalEditorState } from '$states';
import { EntryState, PalGender, type Pal, type Player, type WorkSuitability } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import PalBadge from '../PalBadge.svelte';
import './fixtures/matchMediaPolyfill';

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
		active_skills: [],
		passive_skills: [],
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

const noop = () => {};
const requiredHandlers = { onMove: noop, onAdd: noop, onClone: noop, onDelete: noop };

beforeEach(() => {
	getAppState().selectedPlayer = undefined;
});

describe('PalBadge level chip', () => {
	it('uses levelCap when provided, even with no selected player', () => {
		render(PalBadge, {
			props: { pal: makePal({ level: 20 }), levelCap: 12, ...requiredHandlers }
		});

		expect(screen.getByText('lvl 12')).toBeTruthy();
	});

	it('preserves the selected-player lookup when levelCap is omitted', () => {
		getAppState().selectedPlayer = { level: 10 } as Player;

		render(PalBadge, {
			props: { pal: makePal({ level: 20 }), ...requiredHandlers }
		});

		expect(screen.getByText('lvl 10')).toBeTruthy();
	});

	it('falls back to the pal level when there is no cap and no selected player', () => {
		render(PalBadge, {
			props: { pal: makePal({ level: 20 }), ...requiredHandlers }
		});

		expect(screen.getByText('lvl 20')).toBeTruthy();
	});
});

describe('PalBadge disabled', () => {
	it('suppresses both the click-to-edit path and the info tooltip', async () => {
		const palEditor = getPalEditorState();
		const { container } = render(PalBadge, {
			props: { pal: makePal(), disabled: true, ...requiredHandlers }
		});

		const tooltipTrigger = container.querySelector('[data-tooltip-trigger]');
		expect(tooltipTrigger).not.toBeNull();
		await fireEvent.mouseEnter(tooltipTrigger!);
		expect(document.querySelector('.tooltip-popup')).toBeNull();

		await fireEvent.click(screen.getByRole('button'));
		expect(palEditor.isOpen).toBe(false);
	});

	it('opens the pal editor on click when not disabled', async () => {
		const palEditor = getPalEditorState();
		render(PalBadge, {
			props: { pal: makePal(), ...requiredHandlers }
		});

		await fireEvent.click(screen.getByRole('button'));
		expect(palEditor.isOpen).toBe(true);
	});
});
