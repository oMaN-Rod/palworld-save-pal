<script lang="ts">
	// Renders one Direct-Mode result row: [Parent A] + [Parent B] → [Child].
	// Used for both the single forward answer and each reverse-mode candidate.
	// A `.breed-row` subgrid cell — the enclosing `.breed-list` owns the column
	// template, so icons and operators line up across every row in the list.
	import * as m from '$i18n/messages';
	import type { BreedablePal, DirectResultItem } from '$lib/breeding/types';
	import Plus from '@lucide/svelte/icons/plus';
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import PalSlot from './PalSlot.svelte';
	import GenderRatio from './GenderRatio.svelte';

	let { result, palMap }: { result: DirectResultItem; palMap: Map<string, BreedablePal> } =
		$props();

	const palA = $derived(palMap.get(result.parent_a));
	const palB = $derived(palMap.get(result.parent_b));
	const palChild = $derived(palMap.get(result.child));

	const childDisplay = $derived(result.child_display || palChild?.display_name || result.child);
	const gendered = $derived(!!(result.parent_a_gender || result.parent_b_gender));
</script>

<div
	class="breed-row rounded-sm bg-surface-900/40 border-surface-700/30 hover:border-surface-700/60 border p-3 transition-colors"
>
	<PalSlot
		tribe={result.parent_a}
		display={palA?.display_name}
		characterId={result.parent_a}
		size="md"
		gender={result.parent_a_gender}
	/>

	<span class="breed-op"><Plus size={18} class="text-surface-400" /></span>

	<PalSlot
		tribe={result.parent_b}
		display={palB?.display_name}
		characterId={result.parent_b}
		size="md"
		gender={result.parent_b_gender}
	/>

	<span class="breed-op"><ArrowRight size={20} class="text-primary-400" /></span>

	<div class="flex min-w-0 items-center gap-2">
		<PalSlot tribe={result.child} display={childDisplay} characterId={result.child} size="md" />
		{#if result.combo_type === 'unique'}
			<span class="chip chip-warning shrink-0 px-1.5 py-0 text-[10px]">{m.breeding_special()}</span>
		{/if}
	</div>

	<div class="flex shrink-0 items-center gap-2 pl-2">
		{#if gendered}
			<span class="chip chip-tertiary shrink-0 px-1.5 py-0 text-[10px]"
				>{m.breeding_gender_specific()}</span
			>
		{/if}
		{#if result.child_gender_prob}
			<GenderRatio prob={result.child_gender_prob} />
		{/if}
	</div>
</div>
