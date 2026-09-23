<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import * as m from '$i18n/messages';
	import { layout } from '$utils/layout.svelte';
	import { cn } from '$theme';

	let {
		page = $bindable(1),
		pageCount,
		label,
		windowSize
	}: {
		page: number;
		pageCount: number;
		label: string;
		windowSize?: number;
	} = $props();

	// Sixteen bubbles wrap into six rows on a phone, burying the grid they page.
	const visibleCount = $derived(windowSize ?? (layout.phone ? 3 : 16));

	const windowStart = $derived(
		Math.max(1, Math.min(page - Math.floor(visibleCount / 2), pageCount - visibleCount + 1))
	);
	const windowEnd = $derived(Math.min(windowStart + visibleCount - 1, pageCount));
	const pages = $derived(
		Array.from({ length: windowEnd - windowStart + 1 }, (_, i) => windowStart + i)
	);

	// Overlays render `role="dialog"` only while open, so any match means the grid is covered.
	// The document is checked, not the target: keystrokes can land on `<body>` with a sheet open.
	const EDITABLE_SELECTOR =
		'input, textarea, select, [contenteditable]:not([contenteditable="false"])';
	const OPEN_DIALOG_SELECTOR = 'dialog[open], [role="dialog"], [role="alertdialog"]';

	function goTo(target: number): void {
		if (target < 1 || target > pageCount || target === page) return;
		page = target;
	}

	function isIgnorableKeydown(event: KeyboardEvent): boolean {
		// `shift` is allowed so `Shift+Q` still pages.
		if (event.ctrlKey || event.metaKey || event.altKey) return true;

		const target = event.target;
		if (target instanceof HTMLElement) {
			if (target.isContentEditable) return true;
			if (target.closest(EDITABLE_SELECTOR)) return true;
		}

		return document.querySelector(OPEN_DIALOG_SELECTOR) !== null;
	}

	// Clamps rather than wraps, matching the disabled prev/next buttons.
	function handleKeydown(event: KeyboardEvent): void {
		if (isIgnorableKeydown(event)) return;

		if (event.key === 'ArrowLeft' || event.key === 'q' || event.key === 'Q') {
			goTo(page - 1);
		} else if (event.key === 'ArrowRight' || event.key === 'e' || event.key === 'E') {
			goTo(page + 1);
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<nav aria-label={label} class="flex items-center justify-center gap-4">
	<button
		type="button"
		class="btn-icon preset-outlined-surface-200-800 rounded-full"
		aria-label={m.previous()}
		disabled={page <= 1}
		onclick={() => goTo(page - 1)}
	>
		<Icon icon="tabler:chevron-left" class="size-5" />
	</button>

	<div class="flex flex-wrap items-center justify-center gap-2">
		{#each pages as pageNumber (pageNumber)}
			<button
				type="button"
				class={cn(
					'btn-icon size-8 rounded-full',
					pageNumber === page ? 'bg-primary-500! text-white' : 'bg-surface-800 hover:bg-surface-600'
				)}
				aria-label={`${m.page()} ${pageNumber}`}
				aria-current={pageNumber === page ? 'page' : undefined}
				onclick={() => goTo(pageNumber)}
			>
				{pageNumber}
			</button>
		{/each}
	</div>

	<button
		type="button"
		class="btn-icon preset-outlined-surface-200-800 rounded-full"
		aria-label={m.next()}
		disabled={page >= pageCount}
		onclick={() => goTo(page + 1)}
	>
		<Icon icon="tabler:chevron-right" class="size-5" />
	</button>
</nav>
