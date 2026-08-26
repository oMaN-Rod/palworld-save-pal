<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cn } from '$theme';

	let {
		variant = 'primary',
		class: className,
		children
	}: {
		variant?: 'primary' | 'secondary' | 'tertiary' | 'success' | 'warning' | 'error';
		class?: string;
		children: Snippet;
	} = $props();
</script>

<span class={cn('chip w-fit', `chip-${variant}`, className)}>{@render children()}</span>
