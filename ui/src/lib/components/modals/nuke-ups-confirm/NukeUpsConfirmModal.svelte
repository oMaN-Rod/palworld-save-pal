<script lang="ts">
	import { Card, Input, Tooltip } from '$components/ui';
	import { X, AlertTriangle, Trash2 } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';

	let {
		totalPals = 0,
		closeModal
	} = $props<{
		totalPals: number;
		closeModal: (value: boolean | null) => void;
	}>();

	let modalContainer: HTMLDivElement;
	let confirmationText = $state('');
	let step = $state(1);
	let isValid = $state(false);

	const REQUIRED_TEXT = 'DELETE ALL';

	// Check if confirmation text is valid
	$effect(() => {
		isValid = confirmationText.trim().toUpperCase() === REQUIRED_TEXT;
	});

	function handleCancel() {
		closeModal(false);
	}

	function handleConfirm() {
		if (step === 1) {
			step = 2;
		} else if (step === 2 && isValid) {
			closeModal(true);
		}
	}

	function handleBack() {
		if (step === 2) {
			step = 1;
			confirmationText = '';
		}
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/2.5)] max-w-md">
		<!-- Header with warning icon -->
		<div class="flex items-center gap-3 mb-6">
			<div class="bg-error-500/20 p-2 rounded-full">
				<AlertTriangle class="h-6 w-6 text-error-500" />
			</div>
			<div>
				<h3 class="h3 text-error-500">Nuke UPS Storage</h3>
				<p class="text-sm text-surface-600 dark:text-surface-400">This action is irreversible</p>
			</div>
		</div>

		{#if step === 1}
			<!-- Step 1: Warning and count display -->
			<div class="space-y-4 mb-6">
				<div class="bg-error-50 dark:bg-error-900/20 border border-error-200 dark:border-error-800 rounded-lg p-4">
					<div class="flex items-start gap-3">
						<Trash2 class="h-5 w-5 text-error-500 mt-0.5 flex-shrink-0" />
						<div class="space-y-2">
							<h4 class="font-semibold text-error-700 dark:text-error-300">
								You are about to delete ALL pals from UPS
							</h4>
							<p class="text-sm text-error-600 dark:text-error-400">
								This will permanently delete {totalPals.toLocaleString()} pal{totalPals !== 1 ? 's' : ''} from your Universal Pal Storage. This action cannot be undone.
							</p>
						</div>
					</div>
				</div>

				{#if totalPals === 0}
					<div class="bg-surface-100 dark:bg-surface-800 rounded-lg p-4 text-center">
						<p class="text-surface-600 dark:text-surface-400">
							UPS is already empty. There are no pals to delete.
						</p>
					</div>
				{:else}
					<div class="bg-warning-50 dark:bg-warning-900/20 border border-warning-200 dark:border-warning-800 rounded-lg p-4">
						<h4 class="font-semibold text-warning-700 dark:text-warning-300 mb-2">
							What will be affected:
						</h4>
						<ul class="text-sm text-warning-600 dark:text-warning-400 space-y-1">
							<li>• All {totalPals.toLocaleString()} pals will be permanently deleted</li>
							<li>• All collection counts will be reset to 0</li>
							<li>• Transfer and clone statistics will be updated</li>
							<li>• Operation will be logged for audit purposes</li>
						</ul>
					</div>
				{/if}
			</div>

			<!-- Step 1 buttons -->
			<div class="flex justify-end space-x-4">
				<Tooltip position="bottom">
					<button
						class="btn preset-filled-secondary hover:preset-tonal-secondary"
						onclick={handleCancel}
					>
						<X size={20} />
						<span>Cancel</span>
					</button>
					{#snippet popup()}
						<span>Cancel operation</span>
					{/snippet}
				</Tooltip>
				{#if totalPals > 0}
					<Tooltip position="bottom">
						<button
							class="btn preset-filled-error hover:preset-tonal-error"
							onclick={handleConfirm}
						>
							<AlertTriangle size={20} />
							<span>Continue</span>
						</button>
						{#snippet popup()}
							<span>Proceed to confirmation</span>
						{/snippet}
					</Tooltip>
				{/if}
			</div>
		{:else if step === 2}
			<!-- Step 2: Text confirmation -->
			<div class="space-y-4 mb-6">
				<div class="bg-error-50 dark:bg-error-900/20 border border-error-200 dark:border-error-800 rounded-lg p-4">
					<h4 class="font-semibold text-error-700 dark:text-error-300 mb-3">
						Final Confirmation Required
					</h4>
					<p class="text-sm text-error-600 dark:text-error-400 mb-4">
						To confirm deletion of {totalPals.toLocaleString()} pals, type <span class="font-mono font-bold bg-error-100 dark:bg-error-900 px-1 rounded">{REQUIRED_TEXT}</span> below:
					</p>
					
					<Input
						bind:value={confirmationText}
						placeholder="Type 'DELETE ALL' to confirm"
						inputClass="font-mono text-center {isValid ? 'border-success-500' : 'border-error-500'}"
						autofocus
					/>
					
					{#if confirmationText && !isValid}
						<p class="text-xs text-error-500 mt-2">
							Text must match exactly: {REQUIRED_TEXT}
						</p>
					{/if}
				</div>
			</div>

			<!-- Step 2 buttons -->
			<div class="flex justify-between">
				<Tooltip position="bottom">
					<button
						class="btn preset-filled-secondary hover:preset-tonal-secondary"
						onclick={handleBack}
					>
						<span>Back</span>
					</button>
					{#snippet popup()}
						<span>Go back to previous step</span>
					{/snippet}
				</Tooltip>
				
				<div class="flex space-x-4">
					<Tooltip position="bottom">
						<button
							class="btn preset-filled-secondary hover:preset-tonal-secondary"
							onclick={handleCancel}
						>
							<X size={20} />
							<span>Cancel</span>
						</button>
						{#snippet popup()}
							<span>Cancel operation</span>
						{/snippet}
					</Tooltip>
					<Tooltip position="bottom">
						<button
							class="btn preset-filled-error hover:preset-tonal-error"
							onclick={handleConfirm}
							disabled={!isValid}
						>
							<Trash2 size={20} />
							<span>Delete All</span>
						</button>
						{#snippet popup()}
							<span>{isValid ? 'Confirm deletion' : 'Enter confirmation text'}</span>
						{/snippet}
					</Tooltip>
				</div>
			</div>
		{/if}
	</Card>
</div>