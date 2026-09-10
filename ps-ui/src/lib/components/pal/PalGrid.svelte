<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cn } from '$theme';

	let {
		children,
		class: klass = '',
		id,
		'data-testid': dataTestid
	}: { children: Snippet; class?: string; id?: string; 'data-testid'?: string } = $props();
</script>

<div
	{id}
	data-testid={dataTestid}
	class={cn(
		'grid grid-cols-3 place-items-center gap-4 p-4 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6',
		klass
	)}
>
	{@render children()}
</div>
