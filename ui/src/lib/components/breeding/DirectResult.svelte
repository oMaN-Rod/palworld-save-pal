<script lang="ts">
	// Renders one Direct-Mode result row: [Parent A] + [Parent B] → [Child].
	// Used for both the single forward answer and each reverse-mode candidate.
	import * as m from '$i18n/messages';
	import type { BreedablePal, DirectResultItem } from '$lib/breeding/types';
	import Plus from '@lucide/svelte/icons/plus';
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import Mars from '@lucide/svelte/icons/mars';
	import Venus from '@lucide/svelte/icons/venus';
	import PalSlot from './PalSlot.svelte';

	let { result, palMap }: { result: DirectResultItem; palMap: Map<string, BreedablePal> } =
		$props();

	const palA = $derived(palMap.get(result.parent_a));
	const palB = $derived(palMap.get(result.parent_b));
	const palChild = $derived(palMap.get(result.child));

	const childDisplay = $derived(result.child_display || palChild?.display_name || result.child);
</script>

<div
	class="rounded-sm bg-surface-900/40 border-surface-700/30 hover:border-surface-700/60 flex items-center gap-3 border p-3 transition-colors"
>
	<PalSlot
		tribe={result.parent_a}
		display={palA?.display_name}
		characterId={result.parent_a}
		size="md"
	/>

		<Plus size={18} class="text-surface-400 shrink-0" />

	<PalSlot
		tribe={result.parent_b}
		display={palB?.display_name}
		characterId={result.parent_b}
		size="md"
	/>

		<ArrowRight size={20} class="text-primary-400 shrink-0" />

	<div class="flex min-w-0 flex-1 items-center gap-2">
		<PalSlot tribe={result.child} display={childDisplay} characterId={result.child} size="md" />
		{#if result.combo_type === 'unique'}
			<span class="chip chip-warning shrink-0 px-1.5 py-0 text-[10px]">{m.breeding_special()}</span>
		{/if}
	</div>

	{#if result.child_gender_prob}
		<div
			class="flex shrink-0 items-center gap-1 text-xs"
			title={m.breeding_gender_probability()}
		>
			{#if result.child_gender_prob.male > 0}
				<span class="text-primary-300 flex items-center gap-0.5" title={m.breeding_male()}>
					<Mars size={11} />
					{Math.round(result.child_gender_prob.male * 100)}%
				</span>
			{/if}
			{#if result.child_gender_prob.female > 0}
				<span class="text-tertiary-400 flex items-center gap-0.5" title={m.breeding_female()}>
					<Venus size={11} />
					{Math.round(result.child_gender_prob.female * 100)}%
				</span>
			{/if}
		</div>
	{/if}
</div>
