<script lang="ts">
	import { Card, Input } from '$components/ui';
	import Tooltip from '$components/ui/tooltip/Tooltip.svelte';
	import { Save, X } from 'lucide-svelte';

	let {
		title = '',
		value = $bindable(''),
		closeModal
	} = $props<{
		title?: string;
		value?: string;
		closeModal: (value: any) => void;
	}>();

	function handleClose(value: any) {
		closeModal(value);
	}
</script>

<Card class="bg-surface-500 min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>

	<div class="mt-2 flex flex-row items-center space-x-2">
		<Input inputClass="grow" bind:value />
		<Tooltip position="bottom">
			{#snippet children()}
				<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(value)}>
					<Save />
				</button>
			{/snippet}
			{#snippet popup()}
				<span>Save</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			{#snippet children()}
				<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(null)}>
					<X />
				</button>
			{/snippet}
			{#snippet popup()}
				<span>Cancel</span>
			{/snippet}
		</Tooltip>
	</div>
</Card>
