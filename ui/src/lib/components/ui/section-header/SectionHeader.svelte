<script lang="ts">
	import { cn } from '$theme';
	import type { Snippet } from 'svelte';

	let { text, baseClass, textClass, borderClass, action } = $props<{
		baseClass?: string;
		textClass?: string;
		borderClass?: string;
		text: string;
		action?: Snippet;
	}>();
</script>

<div class={cn('relative inline-block w-full text-start', baseClass)}>
	<span class={cn('relative z-50 items-start px-2 text-lg font-bold', textClass)}>{text}</span>
	<div class={cn('bg-surface-900 absolute inset-x-0 bottom-0 -z-50 h-1/2', borderClass)}></div>
	{#if action}
		<div class="absolute right-0 top-1/2 z-50 -translate-y-1/2">
			{@render action()}
		</div>
	{/if}
</div>
