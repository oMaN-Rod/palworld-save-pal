<script lang="ts">
	import { cn } from '$theme';
	import { Check } from 'lucide-svelte';

	let {
		checked = $bindable(false),
		label = '',
		class: className = '',
		onchange = (checked: boolean) => {}
	} = $props<{
		checked?: boolean;
		label?: string;
		class?: string;
		onchange?: (checked: boolean) => void;
	}>();

	function handleChange(event: Event) {
		const isChecked = (event.target as HTMLInputElement).checked;
		checked = isChecked;
		onchange(isChecked);
	}
</script>

<label class={cn('flex cursor-pointer items-center space-x-2', className)}>
	<div
		class={cn(
			'flex h-5 w-5 items-center justify-center rounded border',
			checked ? 'bg-primary-500 border-primary-500' : 'bg-surface-800 border-surface-600'
		)}
	>
		{#if checked}
			<Check class="h-4 w-4 text-white" />
		{/if}
	</div>
	{#if label}
		<span>{label}</span>
	{/if}
	<input type="checkbox" class="sr-only" {checked} onchange={handleChange} />
</label>
