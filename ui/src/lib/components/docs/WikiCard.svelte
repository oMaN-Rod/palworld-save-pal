<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { WikiIcon } from '$lib/utils/wikiDescriptors';

	type Props = {
		href: string;
		name: string;
		icon?: WikiIcon | null;
		meta?: string | null;
		/** Secondary mono line (e.g. raw key) shown under the name in hub search. */
		subtext?: string | null;
		/** Right-aligned trailing content (e.g. element badges, index). */
		badges?: Snippet;
		/** List mode: compact horizontal card. Grid mode: vertical card. */
		variant?: 'list' | 'grid';
		/** Icon size class override for list mode; defaults to a compact 36px. */
		iconClass?: string;
		/** Icon size class override for grid mode; defaults to 48px. */
		gridIconClass?: string;
	};

	let {
		href,
		name,
		icon = null,
		meta = null,
		subtext = null,
		badges = undefined,
		variant = 'list',
		iconClass = 'h-9 w-9',
		gridIconClass = 'h-12 w-12'
	}: Props = $props();
</script>

{#if variant === 'grid'}
	<a
		{href}
		class="border-surface-800 hover:border-primary-500/50 hover:bg-surface-700 flex flex-col items-center rounded-lg border p-2.5 text-center transition-colors"
	>
		{#if icon}
			<img
				src={icon.src}
				alt=""
				class="shrink-0 object-contain {gridIconClass}"
				style={icon.filter ? `filter: ${icon.filter};` : undefined}
			/>
		{/if}
		<span class="mt-1.5 line-clamp-2 w-full text-sm font-medium">{name}</span>
		{#if meta}
			<span class="text-surface-400 mt-0.5 text-xs">{meta}</span>
		{/if}
		{#if badges}
			<div class="mt-1 flex items-center gap-1">
				{@render badges()}
			</div>
		{/if}
	</a>
{:else}
	<a
		{href}
		class="border-surface-800 hover:border-primary-500/50 hover:bg-surface-700 flex items-center gap-2 rounded-md border p-2 text-left transition-colors"
	>
		{#if icon}
			<img
				src={icon.src}
				alt=""
				class="shrink-0 object-contain {iconClass}"
				style={icon.filter ? `filter: ${icon.filter};` : undefined}
			/>
		{/if}
		<span class="flex min-w-0 flex-1 flex-col leading-tight">
			<span class="truncate text-sm font-medium">{name}</span>
			{#if subtext}
				<span class="text-surface-500 truncate font-mono text-xs">{subtext}</span>
			{:else if meta}
				<span class="text-surface-400 truncate text-xs">{meta}</span>
			{/if}
		</span>
		{#if badges}
			<span class="flex shrink-0 items-center gap-1">
				{@render badges()}
			</span>
		{/if}
	</a>
{/if}
