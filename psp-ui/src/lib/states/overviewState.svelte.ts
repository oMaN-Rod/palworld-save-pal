import { MessageType } from '$lib/types';
import { isReady, send } from '$lib/utils/websocketUtils';
import { persistedState } from 'svelte-persisted-state';
import { getAppState } from './appState.svelte';

/**
 * Overview dashboard view mode, persisted to localStorage — the Minimal/Full
 * split from the reference implementation. Minimal shows only the world-summary
 * tiles and never fetches stats; switching to Full loads the dashboard.
 */
export const overviewViewMode = persistedState<'minimal' | 'expanded'>(
	'psp-overview-mode',
	'minimal'
);

function currentSessionId(): string | null {
	return getAppState().saveFile?.session_id ?? null;
}

/** Wire shape of `get_overview_stats` — mirrors psp-core's `OverviewStats`. */
export interface OverviewStats {
	totals: {
		players: number;
		pals: number;
		creature_pals: number;
		human_npcs: number;
		species: number;
		guilds: number;
		bases: number;
		containers: number;
	};
	traits: {
		boss_pals: number;
		rare_pals: number;
		awakened_pals: number;
	};
	condition: {
		sick_pals: number;
		fainted_pals: number;
	};
	composition: {
		avg_level: number;
		gender: { male: number; female: number; unknown: number };
		level_brackets: { label: string; count: number }[];
		talent_avg: { hp: number; attack: number; defense: number };
		top_passives: { skill: string; count: number }[];
		top_actives: { skill: string; count: number }[];
	};
	top_species: { key: string; count: number }[];
	top_players: {
		uid: string;
		nickname: string;
		level: number | null;
		pal_count: number;
		lucky_count: number;
		avg_pal_level: number | null;
		max_pal_level: number | null;
		total_power: number;
		dps_pal_count: number;
	}[];
	anomalies: {
		pal_count: number;
		danger_count: number;
		by_code: { code: string; count: number }[];
		flagged: {
			instance_id: string;
			character_id: string;
			character_key: string;
			level: number;
			severity: 'danger' | 'warning';
			codes: string[];
			owner_uid: string | null;
			source: 'world' | 'dps';
		}[];
	};
}

/**
 * Client-side cache for the Overview dashboard. Stats are computed on the
 * backend per request (same lazy pattern as `get_pal_summaries`); this state
 * just remembers the last response per session id, so navigating away and
 * back doesn't refetch, while a save switch or eject drops the cache.
 */
class OverviewStateClass {
	stats = $state<OverviewStats | null>(null);
	loading = $state(false);
	error = $state<string | null>(null);
	/** The session id the cached stats belong to. */
	cachedSessionId = $state<string | null>(null);

	reset() {
		this.stats = null;
		this.loading = false;
		this.error = null;
		this.cachedSessionId = null;
	}

	setStats(stats: OverviewStats) {
		this.stats = stats;
		this.loading = false;
		this.error = null;
		this.cachedSessionId = currentSessionId();
	}

	setError(error: string) {
		this.error = error;
		this.loading = false;
	}

	/** Fetches unless a cache for the current session already exists. */
	load(force = false) {
		if (!getAppState().saveFile) return;
		if (this.loading) return;
		if (
			!force &&
			this.stats &&
			this.cachedSessionId &&
			this.cachedSessionId === currentSessionId()
		) {
			return;
		}
		if (!isReady()) return;
		this.loading = true;
		this.error = null;
		send(MessageType.GET_OVERVIEW_STATS);
	}
}

const overviewState = new OverviewStateClass();
export const getOverviewState = () => overviewState;
