<script lang="ts">
	import * as m from '$i18n/messages';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { BreedablePal, Chain } from '$lib/breeding/types';
	import type { TreeNode } from '$lib/breeding/dendrogram/types';
	import type { LayoutMode } from '$lib/breeding/dendrogram/layouts';
	import ChainDendrogram from './ChainDendrogram.svelte';
	import { Slider } from '$components/ui';

	let {
		trees,
		chains = [],
		palMap,
		passiveName = (asset: string) => asset,
		activeIndex = 0,
		onactiveIndexChange,
		graphLayout = 'all-in-one',
		ongraphLayoutChange,
		currentGen = 1,
		oncurrentGenChange,
		maxDepth = 1,
		viewMode = $bindable('dendrogram'),
		onselect
	}: {
		trees: TreeNode[];
		chains?: Chain[];
		palMap: Map<string, BreedablePal>;
		passiveName?: (asset: string) => string;
		activeIndex: number;
		onactiveIndexChange?: (idx: number) => void;
		graphLayout: 'all-in-one' | 'per-gen';
		ongraphLayoutChange?: (val: 'all-in-one' | 'per-gen') => void;
		currentGen?: number;
		oncurrentGenChange?: (val: number) => void;
		maxDepth?: number;
		// Bindable so the chosen layout persists across chain navigation and mode switches.
		viewMode?: LayoutMode;
		onselect?: (node: TreeNode | null) => void;
	} = $props();

	const activeTree = $derived(trees[activeIndex]);
	const totalTrees = $derived(trees.length);
	const activeChain = $derived(chains[activeIndex]);

	const VIEWS: { mode: LayoutMode; icon: string; label: () => string }[] = [
		{ mode: 'dendrogram', icon: 'tabler:git-fork', label: () => m.breeding_view_dendrogram() },
		{ mode: 'smooth', icon: 'tabler:vector-spline', label: () => m.breeding_view_smooth() },
		{ mode: 'columns', icon: 'tabler:columns-3', label: () => m.breeding_view_columns() },
		{ mode: 'radial', icon: 'tabler:atom', label: () => m.breeding_view_radial() }
	];

	function prev() {
		if (activeIndex > 0) onactiveIndexChange?.(activeIndex - 1);
	}
	function next() {
		if (activeIndex < totalTrees - 1) onactiveIndexChange?.(activeIndex + 1);
	}
</script>

<div class="flex h-full min-h-0 flex-col">
	<div
		class="border-surface-700/30 flex shrink-0 items-center justify-between gap-2 border-b px-3 py-1.5"
	>
		<div class="flex min-w-0 items-center gap-2">
			{#if totalTrees > 0}
				<div class="mr-1 flex items-center gap-0.5">
					<button
						class="btn btn-secondary text-surface-400 hover:text-surface-50 rounded-sm p-1 transition-colors disabled:cursor-not-allowed disabled:opacity-30"
						onclick={prev}
						disabled={activeIndex <= 0}
						title={m.breeding_previous()}
					>
						<Icon icon="tabler:chevron-left" size={12} />
					</button>
					<span class="text-surface-400 shrink-0 px-1 font-mono text-xs tabular-nums">
						{totalTrees > 0 ? `${activeIndex + 1}/${totalTrees}` : '0/0'}
					</span>
					<button
						class="btn btn-secondary text-surface-400 hover:text-surface-50 rounded-sm p-1 transition-colors disabled:cursor-not-allowed disabled:opacity-30"
						onclick={next}
						disabled={activeIndex >= totalTrees - 1}
						title={m.breeding_next()}
					>
						<Icon icon="tabler:chevron-right" size={12} />
					</button>
				</div>
			{/if}

			{#if activeChain}
				<Icon icon="tabler:git-merge" size={14} class="text-primary-400 shrink-0" />
				<h3 class="text-surface-50 truncate text-sm font-semibold">
					{palMap.get(activeChain.target)?.display_name ?? activeChain.target}
				</h3>
				<span class="chip chip-primary shrink-0 px-1.5 py-0.5 text-[10px]"
					>{m.breeding_gens({ n: activeChain.generations })}</span
				>
				{#if activeChain.gender_feasible}
					<Icon icon="tabler:circle-check" size={11} class="text-success-400 shrink-0" />
				{:else}
					<Icon icon="tabler:circle-x" size={11} class="text-error-400 shrink-0" />
				{/if}
				{#if activeChain.matched_passives.length}
					<div class="ml-1 flex shrink-0 flex-wrap gap-1">
						{#each activeChain.matched_passives as passive}
							<span class="chip chip-success px-1.5 py-0 text-[10px]">{passiveName(passive)}</span>
						{/each}
					</div>
				{/if}
			{:else if activeTree}
				<Icon icon="tabler:transfer" size={14} class="text-primary-400 shrink-0" />
				<h3 class="text-surface-50 truncate text-sm font-semibold">{activeTree.display}</h3>
			{/if}
		</div>

		<div class="flex shrink-0 items-center gap-1.5">
			<div class="bg-surface-950/50 border-surface-700/30 flex gap-0.5 rounded-sm border p-0.5">
				{#each VIEWS as view (view.mode)}
					<button
						class="rounded-sm p-1 transition-all {viewMode === view.mode
							? 'bg-primary-500/15 text-primary-300 border-primary-500/40 border'
							: 'text-surface-400 hover:text-surface-200 border border-transparent'}"
						onclick={() => (viewMode = view.mode)}
						title={view.label()}
						aria-pressed={viewMode === view.mode}
					>
						<Icon icon={view.icon} size={12} />
					</button>
				{/each}
			</div>

			{#if chains.length > 0 && maxDepth !== undefined && maxDepth > 1}
				<div class="bg-surface-950/50 border-surface-700/30 flex gap-0.5 rounded-sm border p-0.5">
					<button
						class="rounded-sm px-1.5 py-0.5 text-xs font-medium transition-all {graphLayout ===
						'all-in-one'
							? 'bg-primary-500/15 text-primary-300 border-primary-500/40 border'
							: 'text-surface-400 hover:text-surface-200 border border-transparent'}"
						onclick={() => ongraphLayoutChange?.('all-in-one')}
						title={m.breeding_show_all_generations()}>{m.breeding_all()}</button
					>
					<button
						class="rounded-sm px-1.5 py-0.5 text-xs font-medium transition-all {graphLayout ===
						'per-gen'
							? 'bg-primary-500/15 text-primary-300 border-primary-500/40 border'
							: 'text-surface-400 hover:text-surface-200 border border-transparent'}"
						onclick={() => ongraphLayoutChange?.('per-gen')}
						title={m.breeding_show_single_generation()}>{m.breeding_per_gen()}</button
					>
				</div>

				{#if graphLayout === 'per-gen'}
					<div class="flex items-center gap-1">
						<span class="text-surface-400 text-[10px] whitespace-nowrap">{m.breeding_gen()}</span>
						<Slider
							class="w-16"
							size="xs"
							color="primary"
							min={1}
							max={maxDepth}
							value={currentGen}
							label={m.breeding_gen()}
							onchange={(gen) => oncurrentGenChange?.(gen)}
						/>
						<span class="text-surface-50 w-4 text-right font-mono text-xs tabular-nums">
							{currentGen}
						</span>
					</div>
				{/if}
			{/if}
		</div>
	</div>

	{#if activeTree}
		<div class="min-h-0 flex-1">
			<ChainDendrogram
				treeNode={activeTree}
				{palMap}
				{passiveName}
				fullHeight={true}
				layoutMode={viewMode}
				{onselect}
			/>
		</div>
	{:else}
		<div class="text-surface-400 flex flex-1 items-center justify-center text-xs italic">
			{m.breeding_no_tree()}
		</div>
	{/if}
</div>
