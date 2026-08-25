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
});
