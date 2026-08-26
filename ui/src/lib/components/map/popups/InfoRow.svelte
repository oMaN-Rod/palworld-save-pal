<script lang="ts">
	import type { Component, Snippet } from 'svelte';

	let {
		icon: Icon,
		iconClass = 'text-primary-500',
		label,
		value,
		children
	}: {
		icon?: Component<{ class?: string }>;
		iconClass?: string;
		label: string;
		value?: string;
		children?: Snippet;
	} = $props();
</script>

<div class="flex items-start gap-2">
	{#if Icon}
		<Icon class="mt-0.5 h-3.5 w-3.5 shrink-0 {iconClass}" />
	{/if}
	<div class="min-w-0 flex-1">
		<div class="text-primary-500 text-xs font-medium">{label}</div>
		{#if children}
			{@render children()}
		{:else}
			<div class="truncate font-mono text-xs">{value}</div>
		{/if}
	</div>
</div>
