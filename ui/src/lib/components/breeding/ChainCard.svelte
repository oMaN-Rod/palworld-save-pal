<script lang="ts">
// Renders one breeding Chain: header (target + gen count + gender
		// feasibility), source pals (leaves), and ordered breeding steps.
		import * as m from '$i18n/messages';
		import type { BreedablePal, Chain } from '$lib/breeding/types';
		import GitMerge from '@lucide/svelte/icons/git-merge';
		import Plus from '@lucide/svelte/icons/plus';
		import ArrowRight from '@lucide/svelte/icons/arrow-right';
		import CheckCircle2 from '@lucide/svelte/icons/circle-check-big';
		import XCircle from '@lucide/svelte/icons/circle-x';
		import Package from '@lucide/svelte/icons/package';
		import Hand from '@lucide/svelte/icons/hand';
		import Trees from '@lucide/svelte/icons/trees';
		import PalSlot from './PalSlot.svelte';

	let {
		chain,
		palMap,
		passiveName = (asset: string) => asset
	}: {
		chain: Chain;
		palMap: Map<string, BreedablePal>;
		passiveName?: (asset: string) => string;
	} = $props();

	const palFor = (tribe: string) => palMap.get(tribe);

	const sourceMeta = {
		owned: { icon: Package, label: () => m.breeding_owned(), cls: 'chip-blue' },
		selected: { icon: Hand, label: () => m.breeding_selected(), cls: 'chip-green' },
		wild: { icon: Trees, label: () => m.breeding_wild(), cls: 'chip-amber' }
	} as const;

	function srcMeta(type: string) {
		return sourceMeta[type as keyof typeof sourceMeta] ?? sourceMeta.selected;
	}

	const matchedSet = $derived(new Set(chain.matched_passives));
</script>

<div class="card space-y-3">
	<!-- header -->
	<div class="flex items-center justify-between gap-2 flex-wrap">
		<div class="flex items-center gap-2">
			<GitMerge size={16} class="text-primary-400" />
			<h3 class="text-sm font-semibold text-surface-50">
				{palFor(chain.target)?.display_name ?? chain.target}
			</h3>
			<span class="chip text-[10px] px-2 py-0 chip-blue">{m.breeding_gens({ n: chain.generations })}</span>
			{#if chain.gender_feasible}
				<CheckCircle2 size={13} class="text-emerald-400" />
			{:else}
				<XCircle size={13} class="text-rose-400" />
			{/if}
		</div>
		{#if chain.matched_passives.length}
			<div class="flex flex-wrap gap-1">
				{#each chain.matched_passives as passive}
					<span class="chip chip-green text-[9px] px-1.5 py-0">{passiveName(passive)}</span>
				{/each}
			</div>
		{/if}
	</div>

	<!-- sources (leaves) -->
	{#if chain.sources.length}
		<div class="flex flex-wrap gap-2">
			{#each chain.sources as src}
				{@const meta = srcMeta(src.type)}
				{@const SrcIcon = meta.icon}
				<div
					class="flex items-center gap-1.5 px-2 py-1 rounded-4 bg-surface-900/50 border border-surface-700/30"
				>
					<PalSlot
						tribe={src.pal}
						display={src.display}
						characterId={src.raw_character_id ?? src.pal}
						size="sm"
						gender={src.gender}
					/>
<span class="chip text-[9px] px-1.5 py-0 {meta.cls} shrink-0">
							<SrcIcon size={9} class="inline" />{meta.label()}
						</span>
				</div>
			{/each}
		</div>
	{/if}

	<!-- steps -->
	{#if chain.steps.length}
		<div class="space-y-1.5">
			{#each chain.steps as step, i}
				<div class="flex items-center gap-2 p-2 rounded-4 bg-surface-900/30 border border-surface-700/20">
					<span class="text-[9px] text-surface-400 font-mono w-4 shrink-0">{i + 1}</span>
					<PalSlot tribe={step.parent_a} display={palFor(step.parent_a)?.display_name} characterId={step.parent_a} size="sm" />
					<Plus size={11} class="text-surface-400 shrink-0" />
					<PalSlot tribe={step.parent_b} display={palFor(step.parent_b)?.display_name} characterId={step.parent_b} size="sm" />
					<ArrowRight size={13} class="text-primary-400 shrink-0" />
					<PalSlot tribe={step.child} display={palFor(step.child)?.display_name} characterId={step.child} size="sm" />
					{#if step.inherited_passives.length}
						<div class="flex flex-wrap gap-0.5 ml-auto shrink-0">
{#each step.inherited_passives as p}
									<span class="chip text-[9px] px-1.5 py-0 {matchedSet.has(p) ? 'chip-green' : ''}"
										>{passiveName(p)}</span
									>
							{/each}
						</div>
					{/if}
				</div>
			{/each}
		</div>
{:else}
			<p class="text-xs text-surface-400 italic">{m.breeding_target_already_available()}</p>
	{/if}
</div>
