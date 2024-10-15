<script lang="ts">
	import { fade } from 'svelte/transition';
	import { computePosition, flip, shift, offset, arrow, type Placement } from '@floating-ui/dom';
	import { cn } from '$theme';
	import { getComputedColorHex } from '$utils';

	let {
		baseClass = '',
		menuClass = 'bg-surface-700',
		rounded = 'rounded',
		position = 'bottom-start',
		xOffset = 0,
		yOffset = 0,
		items = $bindable(),
		useArrow = true,
		children
	} = $props<{
		baseClass?: string;
		menuClass?: string;
		rounded?: string;
		position?: Placement;
		xOffset?: number;
		yOffset?: number;
		items: Array<{ label: string; onClick: () => void; icon?: any }>;
		useArrow?: boolean;
		children: any;
	}>();

	let open = $state(false);
	let referenceEl: HTMLElement;
	let floatingEl: HTMLElement | null = $state(null);
	let arrowEl: HTMLElement | null = $state(null);
	let floatingArrowColor = getComputedColorHex(`--${menuClass.replace('bg', 'color')}`);

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
				left: `${x + xOffset}px`,
				top: `${y + yOffset}px`
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

	function handleContextMenu(event: MouseEvent) {
		event.preventDefault();
		open = true;
	}

	function handleClickOutside(event: MouseEvent) {
		if (floatingEl && !floatingEl.contains(event.target as Node)) {
			open = false;
		}
	}

	$effect(() => {
		if (open) {
			document.addEventListener('click', handleClickOutside);
		} else {
			document.removeEventListener('click', handleClickOutside);
		}

		return () => {
			document.removeEventListener('click', handleClickOutside);
		};
	});
</script>

<div
	class={baseClass}
	bind:this={referenceEl}
	oncontextmenu={handleContextMenu}
	role="presentation"
>
	{@render children()}
</div>

{#if open}
	<div
		bind:this={floatingEl}
		class={cn('floating context-menu', menuClass, rounded)}
		transition:fade={{ duration: 100 }}
	>
		<ul class="py-2">
			{#each items as item}
				<li>
					<button
						class="hover:bg-surface-600 flex w-full items-center px-4 py-2 text-left"
						onclick={() => {
							item.onClick();
							open = false;
						}}
					>
						{#if item.icon}
							<div class="mr-2"><item.icon /></div>
						{/if}
						{item.label}
					</button>
				</li>
			{/each}
		</ul>
		{#if useArrow}
			<div
				bind:this={arrowEl}
				class="context-menu-arrow"
				style:--arrow-color={floatingArrowColor}
			></div>
		{/if}
	</div>
{/if}

<style>
	.context-menu {
		z-index: 99999;
		position: fixed;
		min-width: 150px;
		box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
	}

	.context-menu-arrow {
		position: absolute;
		width: 8px;
		height: 8px;
		background: var(--arrow-color);
		transform: rotate(45deg);
	}
</style>
