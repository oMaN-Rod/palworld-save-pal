<script lang="ts">
	import type { Snippet } from 'svelte';
	import { fade } from 'svelte/transition';
	import { cn } from '$theme';
	import { BottomSheet } from '$components/ui/sheet';
	import { layout } from '$utils/layout.svelte';

	let {
		sidebar,
		detail,
		detailActive = false,
		sidebarClass = 'sm:w-72 md:w-100',
		class: klass = '',
		detailOnPhone = 'inline',
		detailTitle = '',
		onDetailClose
	}: {
		sidebar: Snippet;
		detail?: Snippet;
		detailActive?: boolean;
		sidebarClass?: string;
		class?: string;
		/** `sheet` moves the detail into a `BottomSheet` on phones. */
		detailOnPhone?: 'inline' | 'sheet';
		detailTitle?: string;
		onDetailClose?: () => void;
	} = $props();

	const asSheet = $derived(detailOnPhone === 'sheet' && layout.phone);

	let snap = $state<'peek' | 'half' | 'tall'>('tall');
</script>

<div class={cn('flex h-full flex-col', klass)}>
	<div class="flex flex-1 flex-col gap-4 overflow-hidden p-4 sm:flex-row">
		<div
			class="hidden transition-all duration-300 sm:block"
			style="flex-grow: {detailActive && !asSheet ? 0 : 1}"
		></div>

		<div
			data-sidebar
			class={cn(
				'max-h-full w-full self-start overflow-y-auto transition-[width] duration-300 sm:flex-none',
				sidebarClass
			)}
		>
			{@render sidebar()}
		</div>

		{#if !asSheet}
			<div class="flex-1 overflow-y-auto">
				{#if detailActive}
					<div in:fade={{ delay: 150, duration: 250 }}>
						{@render detail?.()}
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>

{#if asSheet}
	<BottomSheet
		open={detailActive}
		bind:snap
		snaps={['peek', 'half', 'tall']}
		title={detailTitle}
		onClose={() => onDetailClose?.()}
	>
		{@render detail?.()}
	</BottomSheet>
{/if}
