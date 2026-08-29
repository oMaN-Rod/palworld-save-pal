<script lang="ts">
	import { palsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import * as m from '$i18n/messages';
	import type { OverviewStats } from '$states';

	let { species }: { species: OverviewStats['top_species'] } = $props();

	const leader = $derived(Math.max(1, ...species.map((entry) => entry.count)));

	function name(key: string): string {
		return palsData.getByKey(key)?.localized_name ?? key;
	}

	function icon(key: string): string {
		const isPal = palsData.getByKey(key)?.is_pal ?? true;
		// Paldeck portrait icons (t_*_icon_normal), not the full-body renders.
		return assetLoader.loadMenuImage(key, isPal);
	}
</script>

<div class="card h-full">
	<h3 class="text-surface-400 mb-4 text-xs font-semibold tracking-wider uppercase">
		{m.overview_top_species()}
	</h3>
	<ul class="flex flex-col gap-2">
		{#each species as entry (entry.key)}
			<li class="flex items-center gap-3">
				<img
					src={icon(entry.key)}
					alt={name(entry.key)}
					class="h-9 w-9 shrink-0 rounded-md object-contain"
					loading="lazy"
				/>
				<div class="min-w-0 flex-1">
					<div class="mb-1 flex items-baseline justify-between gap-2">
						<span class="text-surface-100 truncate text-sm font-medium">
							{name(entry.key)}
						</span>
						<span class="text-surface-500 shrink-0 text-xs tabular-nums">
							×{entry.count.toLocaleString()}
						</span>
					</div>
					<div class="bg-surface-900/60 h-1.5 overflow-hidden rounded-full">
						<div
							class="bg-primary-500 h-full rounded-full transition-all"
							style="width: {Math.round((entry.count / leader) * 100)}%"
						></div>
					</div>
				</div>
			</li>
		{:else}
			<li class="text-surface-500 py-4 text-sm">—</li>
		{/each}
	</ul>
</div>
