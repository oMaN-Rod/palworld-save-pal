<script lang="ts">
	import { computePosition, flip, shift, offset, type Placement } from '@floating-ui/dom';
	import { portal } from '$utils';
	import { fade } from 'svelte/transition';
	import { cn } from '$theme';
	import type { Snippet } from 'svelte';

	let {
		children,
		content,
		position = 'bottom',
		class: className = '',
		popoverClass = ''
	}: {
		children: Snippet;
		content: Snippet<[{ close: () => void }]>;
		position?: Placement;
		class?: string;
		popoverClass?: string;
	} = $props();

	let isOpen = $state(false);
	let referenceEl: HTMLElement | null = $state(null);
	let floatingEl: HTMLElement | null = $state(null);

	function toggle() {
		isOpen = !isOpen;
	}

	function close() {
		isOpen = false;
	}

	$effect(() => {
		if (isOpen && referenceEl && floatingEl) {
			computePosition(referenceEl, floatingEl, {
				placement: position,
				middleware: [offset(8), flip(), shift({ padding: 5 })]
			}).then(({ x, y }) => {
				if (floatingEl) {
					Object.assign(floatingEl.style, { left: `${x}px`, top: `${y}px` });
				}
			});
		}
	});

	function handleDocumentClick(event: MouseEvent) {
		if (!isOpen) return;
		const clickTarget = event.target as Node;
		if (referenceEl?.contains(clickTarget) || floatingEl?.contains(clickTarget)) return;
		isOpen = false;
	}

	function handleEscapeKey(event: KeyboardEvent) {
		if (event.key === 'Escape' && isOpen) close();
	}
</script>

<svelte:document onclick={handleDocumentClick} onkeydown={handleEscapeKey} />

<div bind:this={referenceEl} class={className} onclick={toggle} role="none">
	{@render children()}
</div>

{#if isOpen}
	<div
		bind:this={floatingEl}
		{@attach portal()}
		class={cn('bg-surface-800 rounded shadow-xl p-3 min-w-48', popoverClass)}
		style="position: fixed; z-index: 99999;"
		transition:fade={{ duration: 100 }}
		role="dialog"
		aria-modal="true"
	>
		{@render content({ close })}
	</div>
{/if}
