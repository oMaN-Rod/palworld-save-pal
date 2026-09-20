<script lang="ts">
	import { fly, fade } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import type { Snippet } from 'svelte';
	import * as m from '$i18n/messages';
	import { trapFocus } from '$lib/utils/focusTrap';

	import {
		resolveDrag,
		SHEET_SNAP_VH,
		DEFAULT_SNAPS,
		DRAG_MOVE_EPSILON_PX,
		type SheetSnap
	} from './sheetSnap';

	let {
		open = $bindable(false),
		snap = $bindable('peek' as SheetSnap),
		snaps = DEFAULT_SNAPS,
		title,
		onClose,
		children
	}: {
		open?: boolean;
		snap?: SheetSnap;
		snaps?: SheetSnap[];
		title: string;
		onClose: () => void;
		children: Snippet;
	} = $props();

	const titleId = $props.id();

	let dialogEl: HTMLElement | null = $state(null);
	let dragStartY: number | null = null;
	// A drag still emits a click on release, which must not re-toggle the snap.
	let dragHandled = false;

	function handlePointerDown(event: PointerEvent): void {
		dragStartY = event.clientY;
		(event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
	}

	function handlePointerUp(event: PointerEvent): void {
		if (dragStartY === null) return;
		const deltaY = event.clientY - dragStartY;
		dragStartY = null;

		if (Math.abs(deltaY) > DRAG_MOVE_EPSILON_PX) dragHandled = true;

		const next = resolveDrag(snap, deltaY, snaps);
		if (next === snap) return;

		if (next === 'closed') onClose();
		else snap = next;
	}

	function handleToggle(): void {
		if (dragHandled) {
			dragHandled = false;
			return;
		}
		const index = snaps.indexOf(snap);
		snap = snaps[(index + 1) % snaps.length];
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key !== 'Escape') return;
		event.preventDefault();
		onClose();
	}

	$effect(() => {
		if (!open || !dialogEl) return;
		document.addEventListener('keydown', handleKeydown);
		const releaseFocus = trapFocus(dialogEl);
		return () => {
			document.removeEventListener('keydown', handleKeydown);
			releaseFocus();
		};
	});
</script>

{#if open}
	<div
		class="fixed inset-0 z-[50000] bg-black/55"
		data-testid="sheet-backdrop"
		onclick={onClose}
		role="presentation"
		transition:fade={{ duration: 150 }}
	></div>

	<div
		bind:this={dialogEl}
		class="bg-surface-800 border-surface-600 fixed inset-x-0 bottom-0 z-[50001] flex flex-col rounded-t-2xl border-t shadow-2xl"
		style:height="{SHEET_SNAP_VH[snap]}vh"
		style:padding-bottom="env(safe-area-inset-bottom)"
		role="dialog"
		aria-modal="true"
		aria-labelledby={titleId}
		transition:fly={{ y: 400, duration: 220, easing: cubicOut }}
	>
		<button
			type="button"
			class="flex w-full shrink-0 justify-center py-3"
			aria-label={m.resize_sheet()}
			onpointerdown={handlePointerDown}
			onpointerup={handlePointerUp}
			onclick={handleToggle}
		>
			<span class="bg-surface-500 block h-1 w-10 rounded-full"></span>
		</button>

		<h2 id={titleId} class="heading-gradient px-4 pb-2 text-sm font-bold">{title}</h2>

		<div class="min-h-0 flex-1 overflow-y-auto px-4 pb-4">
			{@render children()}
		</div>
	</div>
{/if}
