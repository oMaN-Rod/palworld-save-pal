<script lang="ts">
	import { Button, Card, Input, Tooltip } from '$components/ui';
	import { Save, X } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let {
		title = '',
		value = $bindable(''),
		inputLabel,
		closeModal
	} = $props<{
		title?: string;
		value?: string;
		inputLabel?: string;
		closeModal: (value: any) => void;
	}>();

	let modalContainer: HTMLDivElement;

	function handleClose(value: any) {
		closeModal(value);
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/3)]">
		<h3 class="h3">{title}</h3>

		<div class="mt-2 flex flex-col space-x-2">
			<Input inputClass="grow" bind:value label={inputLabel} />
			<div class="mt-2 flex justify-end">
				<Tooltip position="bottom">
					{#snippet children()}
						<Button
							variant="ghost"
							size="icon"
							onclick={() => handleClose(value)}
							data-modal-primary
						>
							<Save />
						</Button>
					{/snippet}
					{#snippet popup()}
						<span>{c.save}</span>
					{/snippet}
				</Tooltip>
				<Tooltip position="bottom">
					{#snippet children()}
						<Button variant="ghost" size="icon" onclick={() => handleClose(null)}>
							<X />
						</Button>
					{/snippet}
					{#snippet popup()}
						<span>{m.cancel()}</span>
					{/snippet}
				</Tooltip>
			</div>
		</div>
	</Card>
</div>
