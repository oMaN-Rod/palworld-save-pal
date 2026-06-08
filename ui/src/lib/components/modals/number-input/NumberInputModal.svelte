<script lang="ts">
	import { Button, Card, Input, Tooltip } from '$components/ui';
	import { Save, X } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

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
				<Button variant="ghost" size="icon" onclick={() => handleClose(true)} data-modal-primary>
					<Save />
				</Button>
				{#snippet popup()}
					<span>{c.save}</span>
				{/snippet}
			</Tooltip>
			<Tooltip position="bottom">
				<Button variant="ghost" size="icon" onclick={() => handleClose(false)}>
					<X />
				</Button>
				{#snippet popup()}
					<span>{m.cancel()}</span>
				{/snippet}
			</Tooltip>
		</div>
	</Card>
</div>
