<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	// Renders one breeding Chain: header (target + gen count + gender
	// feasibility), source pals (leaves), and ordered breeding steps.
	import * as m from '$i18n/messages';
	import type { BreedablePal, Chain } from '$lib/breeding/types';
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
		owned: { icon: 'tabler:package', label: () => m.breeding_owned(), cls: 'chip-primary' },
		selected: { icon: 'tabler:hand-grab', label: () => m.breeding_selected(), cls: 'chip-success' },
		wild: { icon: 'tabler:trees', label: () => m.breeding_wild(), cls: 'chip-warning' }
	} as const;

	function srcMeta(type: string) {
		return sourceMeta[type as keyof typeof sourceMeta] ?? sourceMeta.selected;
	}

	const matchedSet = $derived(new Set(chain.matched_passives));
</script>

<div class="card space-y-3">
	<!-- header -->
	<div class="flex flex-wrap items-center justify-between gap-2">
		<div class="flex items-center gap-2">
			<Icon icon="tabler:git-merge" size={16} class="text-primary-400" />
			<h3 class="text-surface-50 text-sm font-semibold">
				{palFor(chain.target)?.display_name ?? chain.target}
			</h3>
			<span class="chip chip-primary px-2 py-0 text-xs"
				>{m.breeding_gens({ n: chain.generations })}</span
			>
			{#if chain.gender_feasible}
				<Icon icon="tabler:circle-check" size={13} class="text-success-400" />
			{:else}
				<Icon icon="tabler:circle-x" size={13} class="text-error-400" />
			{/if}
		</div>
		{#if chain.matched_passives.length}
			<div class="flex flex-wrap gap-1">
				{#each chain.matched_passives as passive}
					<span class="chip chip-success px-1.5 py-0 text-[10px]">{passiveName(passive)}</span>
				{/each}
			</div>
		{/if}
	</div>

	<!-- sources (leaves) -->
	{#if chain.sources.length}
		<div class="flex flex-wrap gap-2">
			{#each chain.sources as src}
				{@const meta = srcMeta(src.type)}
				<div
					class="bg-surface-900/50 border-surface-700/30 flex items-center gap-1.5 rounded-sm border px-2 py-1"
				>
					<PalSlot
						tribe={src.pal}
						display={src.display}
						characterId={src.raw_character_id ?? src.pal}
						size="md"
						gender={src.gender}
					/>
					<span class="chip px-1.5 py-0 text-[10px] {meta.cls} shrink-0">
						<Icon icon={meta.icon} size={9} class="inline" />{meta.label()}
					</span>
				</div>
			{/each}
		</div>
	{/if}

	<!-- steps -->
	{#if chain.steps.length}
		<div class="breed-list-numbered">
			{#each chain.steps as step, i}
				<div class="breed-row bg-surface-900/30 border-surface-700/20 rounded-sm border p-2">
					<span class="text-surface-400 shrink-0 font-mono text-[10px]">{i + 1}</span>
					<PalSlot
						tribe={step.parent_a}
						display={palFor(step.parent_a)?.display_name}
						characterId={step.parent_a}
						size="md"
					/>
					<span class="breed-op"
						><Icon icon="tabler:plus" size={16} class="text-primary-400" /></span
					>
					<PalSlot
						tribe={step.parent_b}
						display={palFor(step.parent_b)?.display_name}
						characterId={step.parent_b}
						size="md"
					/>
					<span class="breed-op"
						><Icon icon="tabler:arrow-right" size={18} class="text-primary-400" /></span
					>
					<PalSlot
						tribe={step.child}
						display={palFor(step.child)?.display_name}
						characterId={step.child}
						size="md"
					/>
					<div class="flex shrink-0 flex-wrap justify-end gap-0.5 pl-2">
						{#each step.inherited_passives as p}
							<span class="chip px-1.5 py-0 text-[10px] {matchedSet.has(p) ? 'chip-success' : ''}"
								>{passiveName(p)}</span
							>
						{/each}
					</div>
				</div>
			{/each}
		</div>
	{:else}
		<p class="text-surface-400 text-xs italic">{m.breeding_target_already_available()}</p>
	{/if}
</div>
