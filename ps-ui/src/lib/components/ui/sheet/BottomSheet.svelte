<script lang="ts">
	import { fly, fade } from 'svelte/transition';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { cn } from '$theme';
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
		modal = true,
		children
	}: {
		open?: boolean;
		snap?: SheetSnap;
		snaps?: SheetSnap[];
		title: string;
		onClose: () => void;
		/** Non-modal: no backdrop, the page behind stays usable, and the sheet has its own close button. */
		modal?: boolean;
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
		// A non-modal sheet leaves the page usable, so trapping focus would lock the user out of it.
		const releaseFocus = modal ? trapFocus(dialogEl) : () => {};
		return () => {
			document.removeEventListener('keydown', handleKeydown);
			releaseFocus();
		};
	});
</script>

{#if open}
	{#if modal}
		<div
			class="fixed inset-0 z-[50000] bg-black/55"
			data-testid="sheet-backdrop"
			onclick={onClose}
			role="presentation"
			transition:fade={{ duration: 150 }}
		></div>
	{/if}

	<div
		bind:this={dialogEl}
		class={cn(
			'bg-surface-800 border-surface-600 inset-x-0 bottom-0 flex flex-col rounded-t-2xl border-t shadow-2xl',
			modal ? 'fixed z-[50001]' : 'absolute z-10'
		)}
		style:height="{SHEET_SNAP_VH[snap]}vh"
		style:padding-bottom="env(safe-area-inset-bottom)"
		role="dialog"
		aria-modal={modal ? 'true' : undefined}
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

		<div class="flex items-center justify-between gap-2 px-4 pb-2">
			<h2 id={titleId} class="heading-gradient text-sm font-bold">{title}</h2>
			{#if !modal}
				<button
					type="button"
					class="hover:bg-surface-700 flex size-11 shrink-0 items-center justify-center rounded-lg"
					aria-label={m.close()}
					onclick={onClose}
				>
					<Icon icon="tabler:x" class="size-5" />
				</button>
			{/if}
		</div>

		<div class="min-h-0 flex-1 overflow-y-auto px-4 pb-4">
			{@render children()}
		</div>
	</div>
{/if}
