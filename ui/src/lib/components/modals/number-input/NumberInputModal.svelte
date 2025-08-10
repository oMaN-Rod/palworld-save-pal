<script lang="ts">
	import { Card, Input } from '$components/ui';
	import Tooltip from '$components/ui/tooltip/Tooltip.svelte';
	import { Save, X } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';

	let {
		title = '',
		message = '',
		value = $bindable(0),
		min = 0,
		max = 100,
		closeModal
	} = $props<{
		title?: string;
		message?: string;
		value?: number;
		min?: number;
		max?: number;
		closeModal: (value: any) => void;
	}>();

	let modalContainer: HTMLDivElement;

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? value : null);
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/3)]">
		<h3 class="h3">{title}</h3>
		{#if message}
			<p class="text-sm">{message}</p>
		{/if}

		<div class="mt-2 flex flex-row items-center space-x-2">
			<Input type="number" inputClass="grow" bind:value {min} {max} />
			<Tooltip position="bottom">
				<button
					class="btn hover:bg-secondary-500 px-2"
					onclick={() => handleClose(true)}
					data-modal-primary
				>
					<Save />
				</button>
				{#snippet popup()}
					<span>Save</span>
				{/snippet}
			</Tooltip>
			<Tooltip position="bottom">
				<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(false)}>
					<X />
				</button>
				{#snippet popup()}
					<span>Cancel</span>
				{/snippet}
			</Tooltip>
		</div>
	</Card>
</div>
