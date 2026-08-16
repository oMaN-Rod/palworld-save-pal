<script lang="ts">
	import { fade } from 'svelte/transition';
	import { getModalState } from '$states';
	import { cn } from '$theme';
	import { onMount } from 'svelte';
	import Button from '../button/Button.svelte';

	const modal = getModalState();

	let {
		overlayClass = 'bg-black/60 backdrop-blur-sm',
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

	// Registered and torn down from onMount, which never runs during SSR.
	// onDestroy does run server-side, so cleaning up there would touch `window`.
	onMount(() => {
		window.addEventListener('keydown', handleKeydown);
		return () => window.removeEventListener('keydown', handleKeydown);
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
			<Button
				variant="ghost"
				size="icon"
				class="absolute top-2 right-2 z-10 text-2xl leading-none"
				onclick={() => modal.closeModal()}
			>
				×
			</Button>
			<modal.component {...modal.props} closeModal={modal.closeModal} />
		</div>
	</div>
{/if}

<style>
	.modal-content {
		z-index: 50000;
	}
</style>
