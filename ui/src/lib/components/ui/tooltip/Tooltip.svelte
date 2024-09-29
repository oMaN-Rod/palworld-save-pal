<script lang="ts">
	import { fade } from 'svelte/transition';
	import { computePosition, flip, shift, offset, arrow, type Placement } from '@floating-ui/dom';
	import { cn } from '$theme';
	import { getComputedColorHex } from '$utils';

	let {
		baseClass = '',
		background = 'bg-surface-500',
		rounded = 'rounded',
		popupClass = 'p-4',
		popup,
		position = 'bottom',
		useArrow = true,
		children
	} = $props<{
		baseClass?: string;
		background?: string;
		rounded?: string;
		popupClass?: string;
		popup?: any;
		position?: Placement;
		useArrow?: boolean;
		children: any;
	}>();

	let open = $state(false);
	let referenceEl: HTMLElement;
	let floatingEl: HTMLElement | null = $state(null);
	let arrowEl: HTMLElement | null = $state(null);
	let floatingArrowColor = getComputedColorHex(`--${background.replace('bg', 'color')}`);

	$effect(() => {
		if (open && referenceEl && floatingEl) {
			updatePosition();
		}
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
	onmouseenter={() => (open = true)}
	onmouseleave={() => (open = false)}
	onfocusin={() => (open = true)}
	onfocusout={() => (open = false)}
	role="tooltip"
>
	{@render children()}
</div>

{#if open}
	<div
		bind:this={floatingEl}
		class={cn('floating tooltip-popup', background, popupClass, rounded)}
		transition:fade={{ duration: 100 }}
	>
		{@render popup()}
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
