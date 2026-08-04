<script lang="ts">
	// Renders one Direct-Mode result row: [Parent A] + [Parent B] → [Child].
	// Used for both the single forward answer and each reverse-mode candidate.
	import type { BreedablePal, DirectResultItem } from '$lib/breeding/types';
	import Plus from '@lucide/svelte/icons/plus';
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import Mars from '@lucide/svelte/icons/mars';
	import Venus from '@lucide/svelte/icons/venus';
	import PalSlot from './PalSlot.svelte';

	let {
		result,
		palMap
	}: { result: DirectResultItem; palMap: Map<string, BreedablePal> } = $props();

	const palA = $derived(palMap.get(result.parent_a));
	const palB = $derived(palMap.get(result.parent_b));
	const palChild = $derived(palMap.get(result.child));

	const childDisplay = $derived(result.child_display || palChild?.display_name || result.child);
</script>

<div
	class="flex items-center gap-3 p-3 rounded-4 bg-surface-900/40 border border-surface-700/30 hover:border-surface-700/60 transition-colors"
>
	<PalSlot tribe={result.parent_a} display={palA?.display_name} characterId={result.parent_a} size="sm" />

	<Plus size={14} class="text-surface-400 shrink-0" />

	<PalSlot tribe={result.parent_b} display={palB?.display_name} characterId={result.parent_b} size="sm" />

	<ArrowRight size={16} class="text-primary-400 shrink-0" />

	<div class="flex items-center gap-2 min-w-0 flex-1">
		<PalSlot tribe={result.child} display={childDisplay} characterId={result.child} size="md" />
		{#if result.combo_type === 'unique'}
			<span class="chip chip-amber text-[9px] px-1.5 py-0 shrink-0">Special</span>
		{/if}
	</div>

	{#if result.child_gender_prob}
		<div class="flex items-center gap-1 shrink-0 text-[10px]">
			{#if result.child_gender_prob.male > 0}
				<span class="text-sky-400 flex items-center gap-0.5" title="Male probability">
					<Mars size={11} />
					{Math.round(result.child_gender_prob.male * 100)}%
				</span>
			{/if}
			{#if result.child_gender_prob.female > 0}
				<span class="text-pink-400 flex items-center gap-0.5" title="Female probability">
					<Venus size={11} />
					{Math.round(result.child_gender_prob.female * 100)}%
				</span>
			{/if}
		</div>
	{/if}
</div>
