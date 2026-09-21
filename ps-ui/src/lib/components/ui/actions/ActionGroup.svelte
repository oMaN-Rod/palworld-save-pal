<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { ActionSheet } from '$components/ui/sheet';
	import { layout } from '$utils/layout.svelte';

	import ActionRail from './ActionRail.svelte';
	import type { ActionDescriptor } from './actionDescriptor';

	let {
		actions,
		title,
		subtitle,
		id,
		class: className
	}: {
		actions: ActionDescriptor[];
		title: string;
		subtitle?: string;
		id?: string;
		class?: string;
	} = $props();

	// Owned here so crossing the breakpoint doesn't unmount it with a branch child.
	let sheetOpen = $state(false);

	$effect(() => {
		if (!layout.phone && sheetOpen) sheetOpen = false;
	});
</script>

{#if layout.phone}
	<button
		type="button"
		class="btn-primary fixed right-4 bottom-4 z-[40] flex size-14 items-center justify-center rounded-full shadow-lg"
		style:margin-bottom="env(safe-area-inset-bottom)"
		aria-label={title}
		onclick={() => (sheetOpen = true)}
	>
		<Icon icon="tabler:dots" class="size-6" />
	</button>

	<ActionSheet
		bind:open={sheetOpen}
		{title}
		{subtitle}
		{actions}
		onClose={() => (sheetOpen = false)}
	/>
{:else}
	<ActionRail {actions} {id} class={className} />
{/if}
