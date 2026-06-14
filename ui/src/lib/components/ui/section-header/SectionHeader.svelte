<script lang="ts">
	import { cn } from '$theme';
	import type { Snippet } from 'svelte';

	let { text, baseClass, textClass, borderClass, action }:{
		baseClass?: string;
		textClass?: string;
		borderClass?: string;
		text: string;
		action?: Snippet;
	} = $props();
</script>

<div class={cn('relative inline-block w-full text-start mb-2', baseClass)}>
	<span
		class={cn('relative z-50 items-start px-2 text-lg font-bold', textClass)}
		style="text-shadow: 0 0 20px rgba(0, 210, 255, 0.1);">{text}</span
	>
	<div class={cn('bg-surface-900 absolute inset-x-0 bottom-0 -z-50 h-1/2', borderClass)}></div>
	{#if action}
		<div class="absolute top-1/2 right-0 z-50 -translate-y-1/2">
			{@render action()}
		</div>
	{/if}
</div>
