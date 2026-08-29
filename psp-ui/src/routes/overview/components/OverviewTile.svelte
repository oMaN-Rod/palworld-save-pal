<script lang="ts">
	import { cn } from '$theme';
	import Icon from '$lib/components/ui/icons/Icon.svelte';

	let {
		label,
		value,
		icon,
		accent = 'text-primary-400',
		format = true
	}: {
		label: string;
		value: number | string;
		/** Iconify icon name, e.g. `tabler:users`. */
		icon: string;
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
		<Icon {icon} size={18} class={cn('shrink-0', accent)} />
	</div>
	<span class="text-surface-100 text-2xl font-bold tabular-nums">
		{display}
	</span>
</div>
