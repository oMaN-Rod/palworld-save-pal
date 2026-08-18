<script lang="ts">
	import { cn } from '$theme';
	import type { Component } from 'svelte';

	let {
		label,
		value,
		icon: Icon,
		accent = 'text-primary-400',
		format = true
	}: {
		label: string;
		value: number | string;
		icon: Component<{ class?: string; size?: number }>;
		accent?: string;
		format?: boolean;
	} = $props();

	const display = $derived(
		typeof value === 'number' && format ? value.toLocaleString() : String(value)
	);
</script>

<div class="card card-hover flex flex-col gap-1" aria-label={label}>
	<div class="flex items-center justify-between gap-2">
		<span class="text-surface-400 truncate text-xs font-semibold tracking-wider uppercase">
			{label}
		</span>
		<Icon size={18} class={cn('shrink-0', accent)} />
	</div>
	<span class="text-surface-100 text-2xl font-bold tabular-nums">
		{display}
	</span>
</div>
