<script lang="ts">
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import Crown from '@lucide/svelte/icons/crown';
	import User from '@lucide/svelte/icons/user';
	import type { OverviewStats } from '$states';

	let { players }: { players: OverviewStats['top_players'] } = $props();
</script>

<div class="card h-full">
	<h3 class="text-surface-400 mb-4 text-xs font-semibold tracking-wider uppercase">
		{m.overview_top_players()}
	</h3>
	<ul class="flex flex-col gap-2">
		{#each players as player, index (player.uid)}
			<li class="flex items-center gap-3">
				{#if index === 0 && player.pal_count > 0}
					<Crown size={18} class="text-warning-400 shrink-0" />
				{:else}
					<User size={18} class="text-surface-500 shrink-0" />
				{/if}
				<span
					class={cn(
						'text-surface-500 w-4 shrink-0 text-right text-xs tabular-nums',
						index === 0 && 'text-warning-400'
					)}
				>
					{index + 1}
				</span>
				<div class="min-w-0 flex-1">
					<span class="text-surface-100 block truncate text-sm font-medium">
						{player.nickname}
					</span>
					<span class="text-surface-500 text-xs">
						{m.overview_lv({ level: player.level ?? 0 })}
					</span>
				</div>
				<span
					class="border-surface-600/60 bg-surface-900/60 text-surface-300 shrink-0 rounded-full border px-2.5 py-0.5 text-xs font-medium tabular-nums"
				>
					{m.overview_pal_count({ count: player.pal_count.toLocaleString() })}
				</span>
			</li>
		{:else}
			<li class="text-surface-500 py-4 text-sm">—</li>
		{/each}
	</ul>
</div>
