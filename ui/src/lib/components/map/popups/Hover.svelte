<script lang="ts">
	import type { WorldMapPoint } from '$types';
	import type { Snippet } from 'svelte';
	import { SectionHeader } from '$components/ui';
	import Coords from './Coords.svelte';

	let {
		title,
		subtitle,
		icon,
		action,
		header,
		content,
		coords
	}: {
		title?: string;
		subtitle?: string;
		icon?: Snippet;
		action?: Snippet;
		header?: Snippet;
		content?: Snippet;
		coords?: WorldMapPoint;
	} = $props();
</script>

<div
	class="bg-surface-900 border-surface-700 flex min-w-38 flex-col gap-2 rounded border p-4 shadow-lg"
>
	{#if header}
		{@render header()}
	{:else if title}
		<div class="flex items-start gap-2">
			{#if icon}
				<div class="mt-1 shrink-0">{@render icon()}</div>
			{/if}
			<div class="min-w-0 flex-1">
				<SectionHeader text={title} {action} baseClass={subtitle ? 'mb-0' : undefined} />
				{#if subtitle}
					<span class="block truncate px-2 text-xs font-light">{subtitle}</span>
				{/if}
			</div>
		</div>
	{/if}

	{#if content}
		<div class="flex flex-col gap-2 px-2">{@render content()}</div>
	{/if}

	{#if coords}
		<div class="flex flex-col gap-2 px-2">
			<Coords {coords} />
		</div>
	{/if}
</div>
