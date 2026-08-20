<script lang="ts">
	import * as m from '$i18n/messages';
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

	let tx = $derived(
		Math.min(x + 16, (typeof window !== 'undefined' ? window.innerWidth : 9999) - 280)
	);
	let ty = $derived(Math.max(y - 50, 8));

	const sourceMeta = {
		owned: { icon: Package, label: () => m.breeding_owned(), cls: 'text-primary-400' },
		selected: { icon: Hand, label: () => m.breeding_selected(), cls: 'text-success-400' },
		wild: { icon: Trees, label: () => m.breeding_wild(), cls: 'text-warning-400' }
	} as const;

	function srcMeta(type?: string) {
		if (!type) return null;
		return sourceMeta[type as keyof typeof sourceMeta] ?? null;
	}
</script>

{#if node}
	{@const GenderIcon = node.gender === 'Male' ? Mars : node.gender === 'Female' ? Venus : null}
	<div
		class="rounded-md border-primary-500/40 bg-surface-950/95 pointer-events-none fixed z-50 max-w-[260px] border shadow-xl backdrop-blur-md"
		style="left: {tx}px; top: {ty}px;"
	>
		<div class="flex items-start gap-2 p-2.5">
			<img
				src={assetLoader.loadMenuImage(node.character_id)}
				alt=""
				class="rounded-sm border-surface-600 h-12 w-12 shrink-0 border object-cover"
			/>
			<div class="min-w-0 space-y-0.5">
				<div class="flex items-center gap-1.5">
					<span class="text-surface-50 truncate text-sm font-bold">{node.display}</span>
					{#if GenderIcon}
						<GenderIcon
							size={12}
							class="shrink-0 {node.gender === 'Male' ? 'text-primary-300' : 'text-tertiary-400'}"
				/>
			{/if}
		</div>

		{#if node.isBred && node.stepIndex !== undefined}
			<div class="text-primary-400 text-xs">
				Step {m.breeding_step_bred({ n: node.stepIndex + 1 })}
			</div>
		{:else if srcMeta(node.sourceType)}
			{@const m2 = srcMeta(node.sourceType)!}
			{@const MIcon = m2.icon}
			<div class="text-xs {m2.cls} flex items-center gap-1">
				<MIcon size={10} class="inline" />{m2.label()}
			</div>
		{/if}
		{#if node.isTarget}
			<div class="text-primary-400 flex items-center gap-1 text-xs font-semibold">
						<Target size={10} class="inline" />{m.breeding_target()}
					</div>
				{/if}
			</div>
		</div>

		{#if node.passives.length}
			<div class="border-surface-700/30 flex flex-wrap gap-1 border-t px-2.5 pt-0.5 pb-2.5">
				{#each node.passives as passive}
					<span class="chip px-1.5 py-0 text-[10px]">{passiveName(passive)}</span>
				{/each}
			</div>
		{/if}
	</div>
{/if}
