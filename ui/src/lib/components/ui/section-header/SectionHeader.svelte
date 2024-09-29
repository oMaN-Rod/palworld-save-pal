<script lang="ts">
	import type { Snippet } from 'svelte';

	let { text, action } = $props<{
		text: string;
		action?: Snippet;
	}>();
</script>

<div class="relative inline-block w-full">
	<span class="relative px-2 text-lg font-bold">{text}</span>
	<div class="bg-surface-800/50 absolute inset-x-0 bottom-0 h-1/2 opacity-5 dark:opacity-50"></div>
	{#if action}
		<div class="absolute right-0 top-1/2 z-50 -translate-y-1/2">
			{@render action()}
		</div>
	{/if}
</div>
