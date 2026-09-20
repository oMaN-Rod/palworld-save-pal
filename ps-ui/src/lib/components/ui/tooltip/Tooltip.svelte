<script lang="ts">
	import { fade } from 'svelte/transition';
	import { computePosition, flip, shift, offset, arrow, type Placement } from '@floating-ui/dom';
	import { cn } from '$theme';
	import { getComputedColorHex, portal } from '$utils';
	import { layout } from '$utils/layout.svelte';
	import type { Snippet } from 'svelte';

	let {
		baseClass = '',
		background = 'bg-surface-500',
		rounded = 'rounded-sm',
		popupClass = 'p-4',
		popup,
		label,
		position = 'bottom',
		useArrow = true,
		children,
		disabled = false
	}: {
		baseClass?: string;
		background?: string;
		rounded?: string;
		popupClass?: string;
		popup?: Snippet<[]>;
		label?: string;
		position?: Placement;
		useArrow?: boolean;
		children: Snippet<[]>;
		disabled?: boolean;
	} = $props();

	let open = $state(false);
	let referenceEl: HTMLElement;
	let floatingEl: HTMLElement | null = $state(null);
	let arrowEl: HTMLElement | null = $state(null);
	const floatingArrowColor = $derived(
		getComputedColorHex(`--${background.replace('bg', 'color')}`)
	);

	$effect(() => {
		if (open && referenceEl && floatingEl) {
			updatePosition();
		}
	});

	function closeCoarse(): void {
		open = false;
	}

	// `focusin` opens before `click` fires, so a tap toggles from the `open` snapshotted on `pointerdown`.
	// Keyboard activation has no `pointerdown` and toggles the live value.
	let openBeforeTap = false;
	let pointerFlaggedTap = false;

	function handlePointerDown(): void {
		if (!layout.coarse) return;
		openBeforeTap = open;
		pointerFlaggedTap = true;
	}

	function toggleCoarse(): void {
		if (!layout.coarse) return;
		if (pointerFlaggedTap) {
			open = !openBeforeTap;
			pointerFlaggedTap = false;
		} else {
			open = !open;
		}
	}

	// No hover on a coarse pointer: tapping elsewhere or another trigger must close this one.
	$effect(() => {
		if (!open || !layout.coarse) return;

		function handleOutsidePointerDown(event: PointerEvent): void {
			const target = event.target as Node | null;
			if (target && (referenceEl?.contains(target) || floatingEl?.contains(target))) return;
			closeCoarse();
		}

		document.addEventListener('pointerdown', handleOutsidePointerDown);
		return () => document.removeEventListener('pointerdown', handleOutsidePointerDown);
	});

	async function updatePosition() {
		if (referenceEl && floatingEl) {
			const { x, y, placement, middlewareData } = await computePosition(referenceEl, floatingEl, {
				placement: position,
				middleware: [
					offset(8),
					flip(),
					shift({ padding: 5 }),
					arrow({ element: arrowEl as Element })
				]
			});

			if (!floatingEl) return;

			Object.assign(floatingEl.style, {
				left: `${x}px`,
				top: `${y}px`
			});

			if (useArrow && middlewareData.arrow) {
				const { x: arrowX, y: arrowY } = middlewareData.arrow;
				const staticSide = {
					top: 'bottom',
					right: 'left',
					bottom: 'top',
					left: 'right'
				}[placement.split('-')[0]];

				if (arrowEl) {
					Object.assign(arrowEl.style, {
						left: arrowX != null ? `${arrowX}px` : '',
						top: arrowY != null ? `${arrowY}px` : '',
						right: '',
						bottom: '',
						[staticSide as string]: '-4px'
					});
				}
			}
		}
	}
</script>

<div
	class={baseClass}
	bind:this={referenceEl}
	onmouseenter={() => {
		if (!layout.coarse) open = true;
	}}
	onmouseleave={() => {
		if (!layout.coarse) open = false;
	}}
	onfocusin={() => (open = true)}
	onfocusout={(event) => {
		const next = event.relatedTarget as Node | null;
		if (next && floatingEl?.contains(next)) return;
		open = false;
	}}
	onpointerdown={handlePointerDown}
	onclick={toggleCoarse}
	role="presentation"
	data-tooltip-trigger
>
	{@render children()}
</div>

{#if open && !disabled}
	<div
		bind:this={floatingEl}
		{@attach portal()}
		class={cn('floating tooltip-popup', background, popupClass, rounded)}
		role="tooltip"
		transition:fade={{ duration: 100 }}
	>
		{#if popup}
			{@render popup()}
		{:else if label}
			{label}
		{/if}
		{#if useArrow}
			<div bind:this={arrowEl} class="tooltip-arrow" style:--arrow-color={floatingArrowColor}></div>
		{/if}
	</div>
{/if}

<style>
	.tooltip-popup {
		z-index: 99999;
		position: fixed;
		pointer-events: none;
	}

	.tooltip-arrow {
		position: absolute;
		width: 8px;
		height: 8px;
		background: var(--arrow-color);
		transform: rotate(45deg);
	}
</style>
