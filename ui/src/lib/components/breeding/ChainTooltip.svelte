<script lang="ts">
	// ChainTooltip — floating hover card for a dendrogram node. Absolute HTML
	// overlay clamped to viewport.
	import { assetLoader } from '$lib/utils/assetLoader';
	import Mars from '@lucide/svelte/icons/mars';
	import Venus from '@lucide/svelte/icons/venus';
	import Package from '@lucide/svelte/icons/package';
	import Hand from '@lucide/svelte/icons/hand';
	import Trees from '@lucide/svelte/icons/trees';
	import Target from '@lucide/svelte/icons/target';
	import type { TreeNode } from '$lib/breeding/dendrogram/types';

	let {
		node,
		x,
		y,
		passiveName = (asset: string) => asset
	}: {
		node: TreeNode | null;
		x: number;
		y: number;
		passiveName?: (asset: string) => string;
	} = $props();

	let tx = $derived(Math.min(x + 16, (typeof window !== 'undefined' ? window.innerWidth : 9999) - 280));
	let ty = $derived(Math.max(y - 50, 8));

	const sourceMeta = {
		owned: { icon: Package, label: 'Owned', cls: 'text-primary-400' },
		selected: { icon: Hand, label: 'Selected', cls: 'text-emerald-400' },
		wild: { icon: Trees, label: 'Wild', cls: 'text-amber-400' }
	} as const;

	function srcMeta(type?: string) {
		if (!type) return null;
		return sourceMeta[type as keyof typeof sourceMeta] ?? null;
	}
</script>

{#if node}
	{@const GenderIcon = node.gender === 'Male' ? Mars : node.gender === 'Female' ? Venus : null}
	<div
		class="fixed z-50 pointer-events-none max-w-[260px] rounded-8 border border-primary-500/40 bg-surface-950/95 backdrop-blur-md shadow-xl"
		style="left: {tx}px; top: {ty}px;"
	>
		<div class="flex items-start gap-2 p-2.5">
			<img
				src={assetLoader.loadPalImage(node.character_id)}
				alt=""
				class="w-12 h-12 rounded-6 object-cover border border-surface-600 shrink-0"
			/>
			<div class="min-w-0 space-y-0.5">
				<div class="flex items-center gap-1.5">
					<span class="font-bold text-sm text-surface-50 truncate">{node.display}</span>
					{#if GenderIcon}
						<GenderIcon size={12} class="shrink-0 {node.gender === 'Male' ? 'text-sky-400' : 'text-pink-300'}" />
					{/if}
				</div>
				<div class="text-[10px] text-surface-400 font-mono">{node.tribe}</div>

				{#if node.isBred && node.stepIndex !== undefined}
					<div class="text-[10px] text-cyan-400">Step {node.stepIndex + 1} · Bred</div>
				{:else if srcMeta(node.sourceType)}
					{@const m = srcMeta(node.sourceType)!}
					{@const MIcon = m.icon}
					<div class="text-[10px] {m.cls} flex items-center gap-1">
						<MIcon size={10} class="inline" />{m.label}
					</div>
				{/if}
				{#if node.isTarget}
					<div class="text-[10px] text-primary-400 font-semibold flex items-center gap-1">
						<Target size={10} class="inline" />Target
					</div>
				{/if}
			</div>
		</div>

		{#if node.passives.length}
			<div class="px-2.5 pb-2.5 pt-0.5 flex flex-wrap gap-1 border-t border-surface-700/30">
				{#each node.passives as passive}
					<span class="chip text-[9px] px-1.5 py-0">{passiveName(passive)}</span>
				{/each}
			</div>
		{/if}
	</div>
{/if}
