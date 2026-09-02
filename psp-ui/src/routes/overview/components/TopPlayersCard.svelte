<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import type { OverviewStats } from '$states';
	import {
		LEADERBOARD_METRICS,
		metricValue,
		sortPlayersForMetric,
		type LeaderboardMetric
	} from '$lib/utils/leaderboard.utils';

	let { players }: { players: OverviewStats['top_players'] } = $props();

	/** Rows previewed before the expand toggle, mirroring NeedsReviewCard. */
	const PREVIEW_ROWS = 8;

	let metric = $state<LeaderboardMetric>('pal_count');
	let expanded = $state(false);

	const sorted = $derived(sortPlayersForMetric(players, metric));
	const visible = $derived(expanded ? sorted : sorted.slice(0, PREVIEW_ROWS));

	const metricLabels: Record<LeaderboardMetric, () => string> = {
		pal_count: m.overview_metric_pals,
		level: m.overview_metric_level,
		avg_pal_level: m.overview_metric_avg_level,
		max_pal_level: m.overview_metric_max_level,
		lucky_count: m.overview_metric_lucky,
		total_power: m.overview_metric_power,
		dps_pal_count: m.overview_metric_dps
	};

	function metricPill(player: OverviewStats['top_players'][number]): string {
		// Null metrics (no known level, no readable pals) render as an em-dash
		// rather than a misleading 0.
		switch (metric) {
			case 'pal_count':
				return m.overview_pal_count({ count: player.pal_count.toLocaleString() });
			case 'level':
				return player.level == null
					? '—'
					: m.overview_lv({ level: player.level });
			case 'avg_pal_level':
				return player.avg_pal_level == null
					? '—'
					: m.overview_avg_lv({ level: player.avg_pal_level.toFixed(1) });
			case 'max_pal_level':
				return player.max_pal_level == null
					? '—'
					: m.overview_lv({ level: player.max_pal_level });
			case 'lucky_count':
				return m.overview_lucky_count({ count: player.lucky_count.toLocaleString() });
			case 'total_power':
				return new Intl.NumberFormat(undefined, { notation: 'compact' }).format(player.total_power);
			case 'dps_pal_count':
				return m.overview_dps_count({ count: player.dps_pal_count.toLocaleString() });
		}
	}

	function metricPillClass(value: number): string {
		return cn(
			'border-surface-600/60 bg-surface-900/60 text-surface-300 shrink-0 rounded-full border px-2.5 py-0.5 text-xs font-medium tabular-nums',
			value <= 0 && 'text-surface-500 opacity-60'
		);
	}
</script>

<div class="card h-full">
	<h3 class="text-surface-400 mb-4 text-xs font-semibold tracking-wider uppercase">
		{m.overview_top_players()}
	</h3>

	<!-- Ranking metric selector: one pill per leaderboard view -->
	<div class="mb-4 flex flex-wrap gap-1" role="group" aria-label={m.overview_rank_by()}>
		{#each LEADERBOARD_METRICS as key (key)}
			<button
				type="button"
				class={cn(
					'rounded-sm px-2.5 py-1 text-[11px] font-medium transition-all',
					metric === key
						? 'bg-surface-800 text-surface-50 border-surface-600/60 border shadow-sm'
						: 'text-surface-400 hover:bg-surface-800/60 hover:text-surface-200 border border-transparent'
				)}
				onclick={() => (metric = key)}
			>
				{metricLabels[key]()}
			</button>
		{/each}
	</div>

	<ul class="flex flex-col gap-2">
		{#each visible as player, index (player.uid)}
			{@const value = metricValue(player, metric)}
			<li class="flex items-center gap-3">
				{#if index === 0 && value > 0}
					<Icon icon="tabler:crown" size={18} class="text-warning-400 shrink-0" />
				{:else}
					<Icon icon="tabler:user" size={18} class="text-surface-500 shrink-0" />
				{/if}
				<span
					class={cn(
						'text-surface-500 w-4 shrink-0 text-right text-xs tabular-nums',
						index === 0 && value > 0 && 'text-warning-400'
					)}
				>
					{index + 1}
				</span>
				<div class="min-w-0 flex-1">
					<span class="text-surface-100 block truncate text-sm font-medium">
						{player.nickname}
					</span>
					{#if metric === 'level'}
						<span class="text-surface-500 text-xs">
							{m.overview_pal_count({ count: player.pal_count.toLocaleString() })}
						</span>
					{:else if player.level != null}
						<span class="text-surface-500 text-xs">
							{m.overview_lv({ level: player.level })}
						</span>
					{/if}
				</div>
				<span class={metricPillClass(value)}>
					{metricPill(player)}
				</span>
			</li>
		{:else}
			<li class="text-surface-500 py-4 text-sm">—</li>
		{/each}
	</ul>

	{#if sorted.length > PREVIEW_ROWS}
		<button
			type="button"
			class="text-primary-400 hover:text-primary-300 mt-2 flex w-full items-center justify-center gap-1 text-xs font-medium"
			onclick={() => (expanded = !expanded)}
		>
			<Icon icon={expanded ? 'tabler:chevron-up' : 'tabler:chevron-down'} size={14} />
			{expanded ? m.overview_show_less() : m.show_all()} ({sorted.length.toLocaleString()})
		</button>
	{/if}
</div>
