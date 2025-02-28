<script lang="ts">
	import { cn } from '$theme';

	let {
		onClick,
		label,
		class: _class = '',
		disabled = false,
		children
	} = $props<{
		onClick: () => void;
		label?: string;
		class?: string;
		disabled?: boolean;
		children?: any;
	}>();
</script>

<button
	class={cn(
		'bg-surface-800 text-one-surface hover:ring-secondary-500 relative h-1/2 rounded-sm px-6 py-1 font-semibold hover:ring',
		_class
	)}
	onclick={onClick}
	{disabled}
>
	{#if label}
		<span class="relative z-10">{label}</span>
	{:else}
		{@render children()}
	{/if}
	<span class="bg-surface-600 absolute left-0 top-0 h-0.5 w-0.5"></span>
	<span class="bg-surface-600 absolute right-0 top-0 h-0.5 w-0.5"></span>
	<span class="bg-surface-600 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
	<span class="bg-surface-600 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
</button>
