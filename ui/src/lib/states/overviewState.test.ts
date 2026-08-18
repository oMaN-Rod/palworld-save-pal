import { MessageType } from '$types';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { WSHandlerContext } from '../ws/types';

const mockContext: WSHandlerContext = {
	goto: vi.fn()
};

const mockAppState = {
	saveFile: undefined as undefined | { session_id?: string; world_name: string }
};

const sendMock = vi.fn();
const isReadyMock = vi.fn(() => true);

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (...args: unknown[]) => sendMock(...args),
	isReady: () => isReadyMock()
}));

vi.mock('$states/appState.svelte', () => ({
	getAppState: () => mockAppState
}));

// overviewState.svelte.ts uses $state runes; import through the compiled path.
const { getOverviewState } = await import('./overviewState.svelte');
const { getOverviewStatsHandler } = await import('$lib/ws/handlers/overviewHandler');

const sampleStats = {
	totals: {
		players: 1,
		pals: 2,
		creature_pals: 2,
		human_npcs: 0,
		species: 2,
		guilds: 1,
		bases: 1,
		containers: 3
	},
	traits: { boss_pals: 0, rare_pals: 0, awakened_pals: 0 },
	condition: { sick_pals: 0, fainted_pals: 0 },
	composition: {
		avg_level: 0,
		gender: { male: 0, female: 0, unknown: 0 },
		level_brackets: [],
		talent_avg: { hp: 0, attack: 0, defense: 0 },
		top_passives: [],
		top_actives: []
	},
	top_species: [],
	top_players: [],
	anomalies: {
		pal_count: 0,
		danger_count: 0,
		by_code: [],
		flagged: []
	}
};

beforeEach(() => {
	sendMock.mockClear();
	const overviewState = getOverviewState();
	overviewState.reset();
	mockAppState.saveFile = undefined;
});

describe('overviewState', () => {
	it('does not send without a loaded save', () => {
		getOverviewState().load();
		expect(sendMock).not.toHaveBeenCalled();
	});

	it('sends get_overview_stats once per session', () => {
		mockAppState.saveFile = { session_id: 'session-a', world_name: 'A' };
		const overviewState = getOverviewState();
		overviewState.load();
		expect(sendMock).toHaveBeenCalledTimes(1);
		expect(sendMock).toHaveBeenCalledWith(MessageType.GET_OVERVIEW_STATS);

		getOverviewStatsHandler.handle({ stats: sampleStats }, mockContext);
		expect(overviewState.stats).toEqual(sampleStats);
		expect(overviewState.loading).toBe(false);

		// A second load for the same session is served from the cache.
		overviewState.load();
		expect(sendMock).toHaveBeenCalledTimes(1);

		// A forced refresh always refetches.
		overviewState.load(true);
		expect(sendMock).toHaveBeenCalledTimes(2);
	});

	it('refetches when the session changes (save switch)', () => {
		mockAppState.saveFile = { session_id: 'session-a', world_name: 'A' };
		const overviewState = getOverviewState();
		overviewState.load();
		getOverviewStatsHandler.handle({ stats: sampleStats }, mockContext);

		mockAppState.saveFile = { session_id: 'session-b', world_name: 'B' };
		overviewState.load();
		expect(sendMock).toHaveBeenCalledTimes(2);
		expect(overviewState.loading).toBe(true);
	});

	it('records backend errors without clobbering cached stats semantics', () => {
		mockAppState.saveFile = { session_id: 'session-a', world_name: 'A' };
		const overviewState = getOverviewState();
		overviewState.load();
		getOverviewStatsHandler.handle({ error: 'No save file loaded' }, mockContext);
		expect(overviewState.error).toBe('No save file loaded');
		expect(overviewState.loading).toBe(false);
		expect(overviewState.stats).toBeNull();
	});

	it('is registered for the get_overview_stats message type', () => {
		expect(getOverviewStatsHandler.type).toBe(MessageType.GET_OVERVIEW_STATS);
	});
});
