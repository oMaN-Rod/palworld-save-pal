<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { cn } from '$theme';

	let {
		checked = $bindable(false),
		label = '',
		disabled = false,
		class: className = '',
		onchange = () => {}
	} = $props<{
		checked?: boolean;
		label?: string;
		disabled?: boolean;
		class?: string;
		onchange?: (event: Event) => void;
	}>();
</script>

<label
	class={cn(
		'flex items-center space-x-2',
		disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer',
		className
	)}
>
	<div
		class={cn(
			'flex h-5 w-5 items-center justify-center rounded-sm border',
			checked ? 'bg-primary-500 border-primary-500' : 'bg-surface-800 border-surface-600'
		)}
	>
		{#if checked}
			<Icon icon="tabler:check" class="h-4 w-4 text-white" />
		{/if}
	</div>
	{#if label}
		<span>{label}</span>
	{/if}
	<input
		type="checkbox"
		aria-hidden="true"
		style="display: none;"
		bind:checked
		{disabled}
		{onchange}
	/>
</label>
