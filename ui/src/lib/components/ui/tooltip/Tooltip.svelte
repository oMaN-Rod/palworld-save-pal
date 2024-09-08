<script lang="ts">
	import { fade } from 'svelte/transition';
	import {
		autoUpdate,
		offset,
		flip,
		arrow,
		useFloating,
		FloatingArrow,
		useHover,
		useInteractions,
		useRole,
		useDismiss
	} from '@skeletonlabs/floating-ui-svelte';
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
		position?: 'top' | 'bottom' | 'left' | 'right';
		useArrow?: boolean;
		children: any;
	}>();

	let open = $state(false);
	let elemArrow: HTMLElement | null = $state(null);
	let floatingArrowColor = getComputedColorHex(`--${background.replace('bg', 'color')}`);

	const floating = useFloating({
		whileElementsMounted: autoUpdate,
		get open() {
			return open;
		},
		onOpenChange: (v) => (open = v),
		placement: position,
		get middleware() {
			return [offset(10), flip(), elemArrow && arrow({ element: elemArrow })];
		}
	});

	const role = useRole(floating.context, { role: 'tooltip' });
	const hover = useHover(floating.context, { move: false });
	const dismiss = useDismiss(floating.context);
	const interactions = useInteractions([role, hover, dismiss]);
</script>

<div
	class={baseClass}
	bind:this={floating.elements.reference}
	{...interactions.getReferenceProps()}
>
	{@render children()}
</div>
{#if open}
	<div
		bind:this={floating.elements.floating}
		style={floating.floatingStyles}
		{...interactions.getFloatingProps()}
		class={cn('floating tooltip-popup', background, popupClass, rounded)}
		transition:fade={{ duration: 200 }}
	>
		{@render popup()}
		{#if useArrow}
			<FloatingArrow bind:ref={elemArrow} context={floating.context} fill={floatingArrowColor} />
		{/if}
	</div>
{/if}

<style>
	.tooltip-popup {
		z-index: var(--tooltip-z-index, 2147483647) !important;
	}
</style>
