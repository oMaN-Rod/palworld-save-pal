<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { fade } from 'svelte/transition';
	import { getModalState } from '$states';
	import { cn } from '$theme';
	import { onMount } from 'svelte';
	import Button from '../button/Button.svelte';
	import { m } from '$i18n/messages';

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

	function handleOutsideClick(event: MouseEvent) {
		if (event.target === event.currentTarget) {
			modal.closeModal();
		}
	}

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

{#each modal.stack as entry, depth (entry.id)}
	{@const isTop = depth === modal.stack.length - 1}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class={cn('fixed inset-0 flex items-center justify-center', overlayClass)}
		style="z-index: {50000 + depth * 10}"
		transition:fade={{ duration: 200 }}
		onclick={isTop ? handleOutsideClick : undefined}
		onkeydown={isTop ? handleKeydown : undefined}
		role="dialog"
		aria-modal="true"
		aria-hidden={!isTop}
		inert={!isTop}
		tabindex="-1"
	>
		<div class={cn('relative', contentClass, rounded)}>
			<button
				type="button"
				class="bg-surface-950 text-surface-200 border-surface-700 hover:bg-surface-800 hover:text-surface-50 absolute top-0 left-full z-20 ml-2 flex size-11 items-center justify-center rounded-full border-2 shadow-lg transition-colors"
				aria-label={m.close()}
				onclick={() => modal.closeEntry(entry.id)}
			>
				<Icon icon="tabler:x" size={24} />
			</button>
			<entry.component {...entry.props} closeModal={(value: any) => modal.closeEntry(entry.id, value)} />
		</div>
	</div>
{/each}
