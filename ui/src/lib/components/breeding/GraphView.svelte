<script lang="ts">
	/**
	 * GraphView — wraps the dendrogram for the currently-active item. Supports
	 * chain mode (multiple chains) and Direct mode (simple trees). Features
	 * prev/next navigation and per-gen / all-in-one layout toggle (chain mode).
	 */
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import GitMerge from '@lucide/svelte/icons/git-merge';
	import ArrowRightLeft from '@lucide/svelte/icons/arrow-right-left';
	import CheckCircle2 from '@lucide/svelte/icons/circle-check-big';
	import XCircle from '@lucide/svelte/icons/circle-x';
	import type { BreedablePal, Chain } from '$lib/breeding/types';
	import type { TreeNode } from '$lib/breeding/dendrogram/types';
	import ChainDendrogram from './ChainDendrogram.svelte';

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
		onselect?: (node: TreeNode | null) => void;
	} = $props();

	const activeTree = $derived(trees[activeIndex]);
	const totalTrees = $derived(trees.length);
	const activeChain = $derived(chains[activeIndex]);

	function prev() {
		if (activeIndex > 0) onactiveIndexChange?.(activeIndex - 1);
	}
	function next() {
		if (activeIndex < totalTrees - 1) onactiveIndexChange?.(activeIndex + 1);
	}
</script>

<div class="flex flex-col h-full min-h-0">
	<div class="flex items-center justify-between gap-2 px-3 py-1.5 shrink-0 border-b border-surface-700/30">
		<div class="flex items-center gap-2 min-w-0">
			{#if totalTrees > 0}
				<div class="flex items-center gap-0.5 mr-1">
					<button
						class="btn btn-secondary p-1 rounded-3 text-surface-400 hover:text-surface-50 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
						onclick={prev}
						disabled={activeIndex <= 0}
						title="Previous"
					>
						<ChevronLeft size={12} />
					</button>
					<span class="text-[10px] text-surface-400 font-mono px-1 tabular-nums shrink-0">
						{totalTrees > 0 ? `${activeIndex + 1}/${totalTrees}` : '0/0'}
					</span>
					<button
						class="btn btn-secondary p-1 rounded-3 text-surface-400 hover:text-surface-50 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
						onclick={next}
						disabled={activeIndex >= totalTrees - 1}
						title="Next"
					>
						<ChevronRight size={12} />
					</button>
				</div>
			{/if}

			{#if activeChain}
				<GitMerge size={14} class="text-primary-400 shrink-0" />
				<h3 class="text-sm font-semibold text-surface-50 truncate">
					{palMap.get(activeChain.target)?.display_name ?? activeChain.target}
				</h3>
				<span class="chip text-[9px] px-1.5 py-0.5 chip-blue shrink-0"
					>{activeChain.generations} gen</span
				>
				{#if activeChain.gender_feasible}
					<CheckCircle2 size={11} class="text-emerald-400 shrink-0" />
				{:else}
					<XCircle size={11} class="text-rose-400 shrink-0" />
				{/if}
				{#if activeChain.matched_passives.length}
					<div class="flex flex-wrap gap-1 shrink-0 ml-1">
						{#each activeChain.matched_passives as passive}
							<span class="chip chip-green text-[8px] px-1.5 py-0">{passiveName(passive)}</span>
						{/each}
					</div>
				{/if}
			{:else if activeTree}
				<ArrowRightLeft size={14} class="text-primary-400 shrink-0" />
				<h3 class="text-sm font-semibold text-surface-50 truncate">{activeTree.display}</h3>
			{/if}
		</div>

		{#if chains.length > 0 && maxDepth !== undefined && maxDepth > 1}
			<div class="flex items-center gap-1.5 shrink-0">
				<div class="flex gap-0.5 p-0.5 rounded-3 bg-surface-950/50 border border-surface-700/30">
					<button
						class="px-1.5 py-0.5 rounded-2 text-[9px] font-medium transition-all {graphLayout === 'all-in-one' ? 'bg-primary-500/15 text-primary-300 border border-primary-500/40' : 'text-surface-400 hover:text-surface-200 border border-transparent'}"
						onclick={() => ongraphLayoutChange?.('all-in-one')}
						title="Show all generations">All</button
					>
					<button
						class="px-1.5 py-0.5 rounded-2 text-[9px] font-medium transition-all {graphLayout === 'per-gen' ? 'bg-primary-500/15 text-primary-300 border border-primary-500/40' : 'text-surface-400 hover:text-surface-200 border border-transparent'}"
						onclick={() => ongraphLayoutChange?.('per-gen')}
						title="Show one generation at a time">Per-Gen</button
					>
				</div>

				{#if graphLayout === 'per-gen'}
					<div class="flex items-center gap-1">
						<span class="text-[9px] text-surface-400 whitespace-nowrap">Gen</span>
						<input
							type="range"
							min="1"
							max={maxDepth}
							class="w-16 h-1 accent-primary-500 cursor-pointer"
							value={currentGen}
							oninput={(e) =>
								oncurrentGenChange?.(
									parseInt((e.currentTarget as HTMLInputElement).value) || 1
								)}
						/>
						<span class="text-[10px] text-surface-50 font-mono w-4 text-right tabular-nums"
							>{currentGen}</span
						>
					</div>
				{/if}
			</div>
		{/if}
	</div>

	{#if activeTree}
		<div class="flex-1 min-h-0">
			<ChainDendrogram treeNode={activeTree} {palMap} {passiveName} fullHeight={true} {onselect} />
		</div>
	{:else}
		<div class="flex-1 flex items-center justify-center text-xs text-surface-400 italic">
			No tree to display
		</div>
	{/if}
</div>
