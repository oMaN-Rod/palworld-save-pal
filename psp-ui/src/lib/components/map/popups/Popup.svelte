<script lang="ts">
	import type { WorldMapPoint } from '$types';
	import type { Snippet } from 'svelte';
	import { Card, SectionHeader } from '$components/ui';
	import Coords from './Coords.svelte';

	let {
		title,
		subtitle,
		icon,
		action,
		header,
		content,
		actions,
		coords
	}: {
		title?: string;
		subtitle?: string;
		icon?: Snippet;
		action?: Snippet;
		header?: Snippet;
		content?: Snippet;
		actions?: Snippet;
		coords?: WorldMapPoint;
	} = $props();
</script>

<Card class="bg-surface-900! border-surface-700 min-w-70 border shadow-lg">
	<div class="pointer-events-auto flex flex-col gap-3">
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
			<div class="border-surface-700 flex flex-col gap-2 border-t px-2 pt-3">
				<Coords {coords} />
			</div>
		{/if}

		{#if actions}
			<div class="flex flex-col gap-2">{@render actions()}</div>
		{/if}
	</div>
</Card>
