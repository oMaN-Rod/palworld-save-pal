<script lang="ts">
	import type { Snippet } from 'svelte';
	import { fade } from 'svelte/transition';
	import { cn } from '$theme';

	let {
		sidebar,
		detail,
		detailActive = false,
		sidebarClass = 'sm:w-72 md:w-100',
		class: klass = ''
	}: {
		sidebar: Snippet;
		detail?: Snippet;
		detailActive?: boolean;
		sidebarClass?: string;
		class?: string;
	} = $props();
</script>

<div class={cn('flex h-full flex-col', klass)}>
	<div class="flex flex-1 flex-col gap-4 overflow-hidden p-4 sm:flex-row">
		<div
			class="hidden transition-all duration-300 sm:block"
			style="flex-grow: {detailActive ? 0 : 1}"
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

		<div class="flex-1 overflow-y-auto">
			{#if detailActive}
				<div in:fade={{ delay: 150, duration: 250 }}>
					{@render detail?.()}
				</div>
			{/if}
		</div>
	</div>
</div>
