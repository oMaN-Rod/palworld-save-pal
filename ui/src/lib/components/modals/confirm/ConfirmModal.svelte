<script lang="ts">
	import { Button, Card, Tooltip } from '$components/ui';
	import { X, Check } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let {
		title = 'Confirm Action',
		message = 'Are you sure you want to perform this action?',
		confirmText = 'Confirm',
		cancelText = 'Cancel',
		closeModal
	} = $props<{
		title?: string;
		message?: string;
		confirmText?: string;
		cancelText?: string;
		closeModal: (value: boolean | null) => void;
	}>();

	let modalContainer: HTMLDivElement;

	function handleConfirm() {
		closeModal(true);
	}

	function handleCancel() {
		closeModal(false);
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/3)]">
		<h3 class="h3 mb-4">{title}</h3>
		<p class="mb-6">{message}</p>
		<div class="flex justify-end space-x-4">
			<Tooltip position="bottom">
				<Button variant="secondary" onclick={handleCancel}>
				<X size={20} />
				<span>{cancelText}</span>
			</Button>
				{#snippet popup()}
					<span>{cancelText}</span>
				{/snippet}
			</Tooltip>
			<Tooltip position="bottom">
				<Button variant="primary" onclick={handleConfirm} data-modal-primary>
				<Check size={20} />
				<span>{confirmText}</span>
			</Button>
				{#snippet popup()}
					<span>{confirmText}</span>
				{/snippet}
			</Tooltip>
		</div>
	</Card>
</div>
