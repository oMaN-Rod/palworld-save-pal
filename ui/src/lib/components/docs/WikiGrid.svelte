<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';
	import * as m from '$i18n/messages';

	let {
		items,
		empty = m.docs_no_results(),
		toolbar,
		children
	}: {
		items: T[];
		empty?: string;
		toolbar?: Snippet;
		children: Snippet<[T, number]>;
	} = $props();
</script>

{#if toolbar}
	<div class="mb-4">{@render toolbar()}</div>
{/if}

{#if items.length === 0}
	<div class="text-surface-400 flex items-center justify-center py-12">
		<p>{empty}</p>
	</div>
{:else}
	<div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
		{#each items as item, i (i)}
			{@render children(item, i)}
		{/each}
	</div>
{/if}
