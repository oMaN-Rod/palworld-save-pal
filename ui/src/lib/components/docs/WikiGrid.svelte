<script lang="ts">
	import type { Snippet } from 'svelte';
	import * as m from '$i18n/messages';

	let { items, empty = m.docs_no_results(), children }: {
		items: unknown[];
		empty?: string;
		children: Snippet;
	} = $props();
</script>

{#if items.length === 0}
	<div class="flex items-center justify-center py-12 text-surface-400">
		<p>{empty}</p>
	</div>
{:else}
	<div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
		{@render children()}
	</div>
{/if}
