<script lang="ts">
	import type { Snippet } from 'svelte';

	let { text, baseClass, action } = $props<{
		baseClass?: string;
		text: string;
		action?: Snippet;
	}>();
</script>

<div class="relative inline-block w-full text-start {baseClass}">
	<span class="relative items-start px-2 text-lg font-bold">{text}</span>
	<div class="bg-surface-900 absolute inset-x-0 bottom-0 -z-50 h-1/2"></div>
	{#if action}
		<div class="absolute right-0 top-1/2 z-50 -translate-y-1/2">
			{@render action()}
		</div>
	{/if}
</div>
