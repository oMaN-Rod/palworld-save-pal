<script lang="ts">
	import type { Snippet } from 'svelte';
	import { ratioFromPointer, type SplitOrientation } from './resizableSplit';

	let {
		orientation,
		ratio = $bindable(0.5),
		a,
		b
	}: {
		orientation: SplitOrientation;
		ratio?: number;
		a: Snippet;
		b: Snippet;
	} = $props();

	let container: HTMLDivElement | undefined = $state();
	let dragging = $state(false);

	const horizontal = $derived(orientation === 'horizontal');

	function onPointerDown(event: PointerEvent) {
		if (!container) return;
		dragging = true;
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
		event.preventDefault();
	}

	function onPointerMove(event: PointerEvent) {
		if (!dragging || !container) return;
		const rect = container.getBoundingClientRect();
		ratio = horizontal
			? ratioFromPointer(event.clientX, rect.left, rect.width)
			: ratioFromPointer(event.clientY, rect.top, rect.height);
	}

	function onPointerUp(event: PointerEvent) {
		if (!dragging) return;
		dragging = false;
		(event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
	}
</script>

<div
	bind:this={container}
	class="flex h-full min-h-0 w-full min-w-0 {horizontal ? 'flex-row' : 'flex-col'} {dragging
		? 'select-none'
		: ''}"
>
	<div class="min-h-0 min-w-0 overflow-hidden" style="flex: 0 0 {ratio * 100}%">
		{@render a()}
	</div>
	<div
		class="bg-surface-700 hover:bg-secondary-500 shrink-0 {horizontal
			? 'w-1.5 cursor-col-resize'
			: 'h-1.5 cursor-row-resize'} {dragging ? 'bg-secondary-500' : ''}"
		role="separator"
		aria-orientation={horizontal ? 'vertical' : 'horizontal'}
		onpointerdown={onPointerDown}
		onpointermove={onPointerMove}
		onpointerup={onPointerUp}
	></div>
	<div class="min-h-0 min-w-0 flex-1 overflow-hidden">
		{@render b()}
	</div>
</div>
