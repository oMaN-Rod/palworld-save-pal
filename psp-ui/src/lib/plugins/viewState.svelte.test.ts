import { describe, expect, it, vi, beforeEach } from 'vitest';
import type { PluginCommand } from '$types';

const sendAndWait = vi.fn();
vi.mock('$lib/utils/websocketUtils', () => ({
	send: vi.fn(),
	sendAndWait: (...args: unknown[]) => sendAndWait(...args)
}));

const { PluginViewState } = await import('./viewState.svelte');

const SCAN: PluginCommand = {
	id: 'scan',
	title: 'Scan',
	description: null,
	destructive: false,
	params: [
		{
			id: 'min_level',
			type: 'int',
			label: 'Min',
			description: null,
			default: 7,
			min: null,
			max: null,
			options: [],
			entity: null
		}
	]
};

const UI = [
	{
		widgets: [
			{ type: 'number_input', id: 'min_level', label: 'Min' },
			{ type: 'entity_select', id: 'who', entity: 'player', label: 'Who' },
			{ type: 'table', id: 'rows', from: 'scan', selectable: true }
		]
	}
];

beforeEach(() => {
	sendAndWait.mockReset();
});

describe('PluginViewState', () => {
	it('seeds its inputs from the params its widgets feed', () => {
		const state = new PluginViewState(UI, [SCAN]);
		expect(state.inputs.min_level).toBe(7);
	});

	it('keeps a result under the command that produced it', () => {
		const state = new PluginViewState(UI, [SCAN]);
		state.recordResult('scan', { pals: [{ id: 'a' }] });
		expect(state.results.scan).toEqual({ pals: [{ id: 'a' }] });
	});

	/// Re-running the source command replaces a table's contents, so
	/// a selection made over the old rows can no longer mean anything.
	it('clears a selectable table selection when its own command runs again', () => {
		const state = new PluginViewState(UI, [SCAN]);
		state.setSelection('rows', ['a', 'b']);
		state.recordResult('scan', { pals: [] });
		expect(state.selections.rows).toEqual([]);
	});

	it('leaves a selection alone when a different command runs', () => {
		const state = new PluginViewState(UI, [SCAN]);
		state.setSelection('rows', ['a']);
		state.recordResult('other', {});
		expect(state.selections.rows).toEqual(['a']);
	});

	/// A result persists until its own command runs again, so changing
	/// an input must not empty the table underneath it.
	it('keeps a result when an input changes', () => {
		const state = new PluginViewState(UI, [SCAN]);
		state.recordResult('scan', { pals: [{ id: 'a' }] });
		state.setValue('min_level', 99);
		expect(state.results.scan).toEqual({ pals: [{ id: 'a' }] });
	});

	it('toggles a row in and out of a selection', () => {
		const state = new PluginViewState(UI, [SCAN]);
		state.toggleRow('rows', 'a');
		state.toggleRow('rows', 'b');
		expect(state.selections.rows).toEqual(['a', 'b']);
		state.toggleRow('rows', 'a');
		expect(state.selections.rows).toEqual(['b']);
	});

	it('asks for every entity kind its view uses, in one request', async () => {
		sendAndWait.mockResolvedValue({ entities: { player: { options: [{ id: 'p', label: 'P' }], total: 1 } } });
		const state = new PluginViewState(UI, [SCAN]);
		await state.loadEntities();
		expect(sendAndWait).toHaveBeenCalledTimes(1);
		expect(sendAndWait.mock.calls[0][1]).toEqual({ kinds: ['player'] });
		expect(state.optionsFor('player').options).toEqual([{ id: 'p', label: 'P' }]);
	});

	it('asks for nothing when no widget needs an entity', async () => {
		const state = new PluginViewState([{ widgets: [{ type: 'text', text: 'hi' }] }], [SCAN]);
		await state.loadEntities();
		expect(sendAndWait).not.toHaveBeenCalled();
	});

	it('renders an empty option list rather than throwing when the host answers nothing', async () => {
		sendAndWait.mockResolvedValue({});
		const state = new PluginViewState(UI, [SCAN]);
		await state.loadEntities();
		expect(state.optionsFor('player')).toEqual({ options: [], total: 0 });
	});

	it('survives a failed entity request', async () => {
		sendAndWait.mockRejectedValue(new Error('no save'));
		const state = new PluginViewState(UI, [SCAN]);
		await expect(state.loadEntities()).resolves.toBeUndefined();
		expect(state.optionsFor('player')).toEqual({ options: [], total: 0 });
	});

	/// A widget that re-emits the value it was given must not count as a change.
	/// Replacing `inputs` re-renders the widget, which re-mints the change
	/// handler it passed down, which fires the handler again -- a cycle that
	/// only ends when Svelte aborts the whole update with
	/// `effect_update_depth_exceeded`.
	it('leaves its inputs untouched when a value is set to what it already holds', () => {
		const state = new PluginViewState(UI, [SCAN]);
		const before = state.inputs;
		state.setValue('min_level', 7);
		expect(state.inputs).toBe(before);
	});

	it('still replaces its inputs when the value genuinely changes', () => {
		const state = new PluginViewState(UI, [SCAN]);
		const before = state.inputs;
		state.setValue('min_level', 8);
		expect(state.inputs).not.toBe(before);
		expect(state.inputs.min_level).toBe(8);
	});

	it('treats setting a key it has never held as a change', () => {
		const state = new PluginViewState(UI, [SCAN]);
		const before = state.inputs;
		state.setValue('brand_new', '');
		expect(state.inputs).not.toBe(before);
		expect(state.inputs.brand_new).toBe('');
	});

	/// `sendAndWait` correlates replies by message type through a single pending
	/// slot, so a second request of the same type overwrites the first's
	/// resolver: one caller then waits forever and the stray reply reaches no
	/// one. Two effects legitimately ask for entities at startup, so the guard
	/// belongs here rather than at each call site.
	it('does not issue a second entity request while one is in flight', async () => {
		let settle!: (value: unknown) => void;
		sendAndWait.mockReturnValue(
			new Promise((resolve) => {
				settle = resolve;
			})
		);
		const state = new PluginViewState(UI, [SCAN]);

		const first = state.loadEntities();
		const second = state.loadEntities();
		expect(sendAndWait).toHaveBeenCalledTimes(1);

		settle({ entities: { player: { options: [{ id: 'p', label: 'P' }], total: 1 } } });
		await Promise.all([first, second]);
		expect(state.optionsFor('player').options).toEqual([{ id: 'p', label: 'P' }]);
	});

	it('can load entities again once the first load has settled', async () => {
		sendAndWait.mockResolvedValue({ entities: {} });
		const state = new PluginViewState(UI, [SCAN]);
		await state.loadEntities();
		await state.loadEntities();
		expect(sendAndWait).toHaveBeenCalledTimes(2);
	});

	it('can load entities again after a failed load', async () => {
		sendAndWait.mockRejectedValueOnce(new Error('no save'));
		await (async () => {})();
		const state = new PluginViewState(UI, [SCAN]);
		await state.loadEntities();
		sendAndWait.mockResolvedValue({
			entities: { player: { options: [{ id: 'p', label: 'P' }], total: 1 } }
		});
		await state.loadEntities();
		expect(state.optionsFor('player').total).toBe(1);
	});
});
