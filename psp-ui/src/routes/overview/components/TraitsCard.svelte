<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import { staticIcons } from '$types/icons';
	import type { OverviewStats } from '$states';

	let {
		traits,
		condition
	}: { traits: OverviewStats['traits']; condition: OverviewStats['condition'] } = $props();

	const healthy = $derived(condition.sick_pals === 0 && condition.fainted_pals === 0);
</script>

<div class="card">
	<div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
		<div class="bg-surface-900/40 flex flex-col items-center gap-2 rounded-md p-3">
			<img src={staticIcons.alphaIcon} alt={m.boss()} class="h-9 w-9 object-contain" />
			<span class="text-surface-500 text-xs">{m.boss()}</span>
			<span class="text-surface-100 text-xl font-bold tabular-nums">
				{traits.boss_pals.toLocaleString()}
			</span>
		</div>
		<div class="bg-surface-900/40 flex flex-col items-center gap-2 rounded-md p-3">
			<img src={staticIcons.luckyIcon} alt={m.lucky()} class="h-9 w-9 object-contain" />
			<span class="text-surface-500 text-xs">{m.lucky()}</span>
			<span class="text-surface-100 text-xl font-bold tabular-nums">
				{traits.rare_pals.toLocaleString()}
			</span>
		</div>
		<div class="bg-surface-900/40 flex flex-col items-center gap-2 rounded-md p-3">
			<img src={staticIcons.awakeningIcon} alt={m.awakened()} class="h-9 w-9 object-contain" />
			<span class="text-surface-500 text-xs">{m.awakened()}</span>
			<span class="text-surface-100 text-xl font-bold tabular-nums">
				{traits.awakened_pals.toLocaleString()}
			</span>
		</div>
		<div
			class={cn(
				'bg-surface-900/40 flex flex-col items-center gap-2 rounded-md p-3',
				condition.sick_pals > 0 && 'border-warning-500/40 bg-warning-500/10 border'
			)}
		>
			<Icon
				icon="tabler:temperature"
				size={36}
				class={condition.sick_pals > 0 ? 'text-warning-400' : 'text-surface-500'}
			/>
			<span class="text-surface-500 text-xs">{m.overview_sick()}</span>
			<span class="text-surface-100 text-xl font-bold tabular-nums">
				{condition.sick_pals.toLocaleString()}
			</span>
		</div>
		<div
			class={cn(
				'bg-surface-900/40 flex flex-col items-center gap-2 rounded-md p-3',
				condition.fainted_pals > 0 && 'border-error-500/40 bg-error-500/10 border'
			)}
		>
			<Icon
				icon="tabler:skull"
				size={36}
				class={condition.fainted_pals > 0 ? 'text-error-400' : 'text-surface-500'}
			/>
			<span class="text-surface-500 text-xs">{m.overview_fainted()}</span>
			<span class="text-surface-100 text-xl font-bold tabular-nums">
				{condition.fainted_pals.toLocaleString()}
			</span>
		</div>
	</div>
	{#if healthy}
		<div
			class="border-success-500/40 bg-success-500/10 text-success-300 mt-4 flex items-center justify-center gap-2 rounded-md border px-4 py-3 text-sm font-medium"
		>
			<Icon icon="tabler:shield-check" size={18} />
			{m.overview_all_healthy()}
		</div>
	{/if}
</div>
