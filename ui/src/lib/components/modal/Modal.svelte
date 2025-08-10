<script lang="ts">
	import { fade } from 'svelte/transition';
	import { getModalState } from '$states';
	import { cn } from '$theme';
	import { onMount, onDestroy } from 'svelte';

	const modal = getModalState();

	let {
		overlayClass = 'bg-black/50',
		contentClass = '',
		rounded = 'rounded-sm',
		children
	} = $props<{
		overlayClass?: string;
		contentClass?: string;
		rounded?: string;
		children: any;
	}>();

	// Function to handle clicks outside the dialog
	function handleOutsideClick(event: MouseEvent) {
		// If the click target is the overlay (not the dialog content)
		if (event.target === event.currentTarget) {
			modal.closeModal();
		}
	}

	// Function to handle key presses
	function handleKeydown(event: KeyboardEvent) {
		if (!modal.isOpen) return;

		if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			modal.closeModal();
			return;
		}

		if (event.key === 'Enter') {
			event.preventDefault();
			event.stopPropagation();

			const modalElement = event.currentTarget as HTMLElement;
			const primaryButton = modalElement?.querySelector(
				'[data-modal-primary]'
			) as HTMLButtonElement;
			if (primaryButton && !primaryButton.disabled) {
				primaryButton.click();
			}
		}
	}

	// Set up the keydown event listener when the component mounts
	onMount(() => {
		window.addEventListener('keydown', handleKeydown);
	});

	// Clean up the event listener when the component is destroyed
	onDestroy(() => {
		window.removeEventListener('keydown', handleKeydown);
	});
</script>

<div>
	{@render children()}
</div>

{#if modal.isOpen}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class={cn('modal-content fixed inset-0 flex items-center justify-center', overlayClass)}
		transition:fade={{ duration: 200 }}
		onclick={handleOutsideClick}
		onkeydown={handleKeydown}
		role="dialog"
		aria-modal="true"
		tabindex="-1"
	>
		<div class={cn('relative', contentClass, rounded)}>
			<button
				class="absolute right-2 top-2 z-10 text-2xl leading-none hover:opacity-75"
				onclick={() => modal.closeModal()}
			>
				×
			</button>
			<modal.component {...modal.props} closeModal={modal.closeModal} />
		</div>
	</div>
{/if}

<style>
	.modal-content {
		z-index: 2147483647;
	}
</style>
