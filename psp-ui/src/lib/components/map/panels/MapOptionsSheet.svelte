<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { SHEET_SNAP_VH, resolveDrag, type SheetSnap } from '../state/mapSheet';
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import type { Snippet } from 'svelte';
	import * as m from '$i18n/messages';

	let {
		snap = $bindable('peek'),
		title,
		onClose,
		children
	}: {
		snap?: SheetSnap;
		title: string;
		onClose: () => void;
		children: Snippet;
	} = $props();

	let dragStartY: number | null = null;
	// A drag still emits a click on release, which would immediately undo the snap
	// the drag just chose. Set on any release that moved the sheet, consumed by the
	// click that follows it.
	let dragHandled = false;

	function handlePointerDown(event: PointerEvent) {
		dragStartY = event.clientY;
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
	}

	function handlePointerUp(event: PointerEvent) {
		if (dragStartY === null) return;
		const deltaY = event.clientY - dragStartY;
		dragStartY = null;

		const next = resolveDrag(snap, deltaY);
		if (next === snap) return;

		dragHandled = true;
		if (next === 'closed') onClose();
		else snap = next;
	}

	// The handle is the drag affordance, but it must also work from a keyboard and
	// for anyone who taps rather than drags: a plain activation toggles the snap.
	function handleToggle() {
		if (dragHandled) {
			dragHandled = false;
			return;
		}
		snap = snap === 'tall' ? 'peek' : 'tall';
	}
</script>

<aside
	class="bg-surface-900/95 absolute inset-x-0 bottom-0 z-10 flex flex-col rounded-t-xl shadow-lg backdrop-blur-sm"
	style:height="{SHEET_SNAP_VH[snap]}vh"
	style:padding-bottom="env(safe-area-inset-bottom)"
	style:transition="height 200ms cubic-bezier(0.33, 1, 0.68, 1)"
	transition:fly={{ y: 400, duration: 300, easing: cubicOut }}
	aria-label={title}
>
	<div class="relative flex min-h-11 items-center justify-between gap-2 px-3">
		<span class="heading-gradient text-sm font-bold">{title}</span>

		<!-- Centred over the row rather than laid out in it, so the title and close
		     button keep the edges and the grab bar still reads as the sheet's middle. -->
		<button
			type="button"
			class="absolute inset-y-0 left-1/2 flex w-24 -translate-x-1/2 cursor-grab touch-none items-center justify-center"
			aria-label={snap === 'tall' ? m.map_sheet_collapse() : m.map_sheet_expand()}
			onpointerdown={handlePointerDown}
			onpointerup={handlePointerUp}
			onpointercancel={() => (dragStartY = null)}
			onclick={handleToggle}
		>
			<span class="bg-surface-600 h-1 w-10 rounded-full"></span>
		</button>

		<button
			type="button"
			class="hover:bg-surface-800 flex min-h-11 min-w-11 items-center justify-center rounded-lg"
			aria-label={m.close()}
			onclick={onClose}
		>
			<Icon icon="tabler:x" class="h-5 w-5" />
		</button>
	</div>

	{@render children()}
</aside>
