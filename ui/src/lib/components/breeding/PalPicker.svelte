<script lang="ts">
	// Searchable pal selector. Reads the shared breedable-pal list from the
	// breeding backend (via sendAndWait). Renders a dropdown with icon + name +
	// rarity, and emits the chosen tribe via onselect.
	//
	// Audit fix #1: uses the page-provided `pals` array instead of refetching
	// per mount (the PalSavTools bug where N pickers = N fetches).
	//
	// The dropdown is portaled to <body> and positioned with `position: fixed`
	// via Floating UI. Plain `absolute z-50` inside the page fails because the
	// trigger sits inside stacking contexts (.card's backdrop-filter, panels'
	// overflow) that trap the z-index and clip the menu — so the header, tab
	// pills, or result cards painted later overlap it.
	import { assetLoader } from '$lib/utils/assetLoader';
	import { computePosition, flip, shift, offset, autoUpdate } from '@floating-ui/dom';
	import { portal } from '$utils';
	import Search from '@lucide/svelte/icons/search';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import type { BreedablePal } from '$lib/breeding/types';

	let {
		value = null,
		placeholder = 'Select a pal…',
		onselect,
		exclude = [],
		pals = []
	}: {
		value?: string | null;
		placeholder?: string;
		onselect?: (tribe: string, pal: BreedablePal) => void;
		exclude?: string[];
		pals?: BreedablePal[];
	} = $props();

	let open = $state(false);
	let query = $state('');
	let containerEl: HTMLDivElement = $state(null!);
	let triggerEl: HTMLButtonElement = $state(null!);
	let floatingEl: HTMLDivElement = $state(null!);
	let cleanup: (() => void) | null = null;

	const excludeSet = $derived(new Set(exclude));
	const filtered = $derived.by(() => {
		const q = query.trim().toLowerCase();
		let result = pals;
		if (q) {
			result = result.filter(
				(p) => p.display_name.toLowerCase().includes(q) || p.tribe.toLowerCase().includes(q)
			);
		}
		if (excludeSet.size) {
			result = result.filter((p) => !excludeSet.has(p.tribe));
		}
		return result;
	});

	const selectedPal = $derived(pals.find((p) => p.tribe === value) || null);

	function pick(p: BreedablePal) {
		open = false;
		query = '';
		onselect?.(p.tribe, p);
	}

	function handleClickOutside(e: MouseEvent) {
		if (!containerEl?.contains(e.target as Node) && !floatingEl?.contains(e.target as Node)) {
			open = false;
		}
	}

	function handleEscape(e: KeyboardEvent) {
		if (e.key === 'Escape' && open) open = false;
	}

	// Position the portaled menu under the trigger, tracking scroll/resize.
	$effect(() => {
		if (!open || !triggerEl || !floatingEl) return;
		query = '';
		cleanup?.();
		const update = () => {
			if (!triggerEl || !floatingEl) return;
			computePosition(triggerEl, floatingEl, {
				placement: 'bottom-start',
				strategy: 'fixed',
				middleware: [offset(4), flip(), shift({ padding: 6 })]
			}).then(({ x, y }) => {
				Object.assign(floatingEl.style, {
					left: `${x}px`,
					top: `${y}px`,
					width: `${triggerEl.offsetWidth}px`
				});
			});
		};
		update();
		// autoUpdate keeps the menu glued to the trigger through scroll/resize.
		cleanup = autoUpdate(triggerEl, floatingEl, update);
		return () => {
			cleanup?.();
			cleanup = null;
		};
	});

	$effect(() => {
		document.removeEventListener('mousedown', handleClickOutside);
		document.removeEventListener('keydown', handleEscape);
		if (open) {
			document.addEventListener('mousedown', handleClickOutside);
			document.addEventListener('keydown', handleEscape);
		}
		return () => {
			document.removeEventListener('mousedown', handleClickOutside);
			document.removeEventListener('keydown', handleEscape);
		};
	});
</script>

<div class="relative" bind:this={containerEl}>
	<button
		type="button"
		bind:this={triggerEl}
		class="input flex items-center gap-2 text-left cursor-pointer hover:border-primary-500/50 transition-colors"
		onclick={() => (open = !open)}
		aria-haspopup="listbox"
		aria-expanded={open}
	>
		{#if selectedPal}
			<img
				src={assetLoader.loadPalImage(selectedPal.tribe)}
				alt={selectedPal.display_name}
				class="w-5 h-5 object-contain rounded-2 bg-surface-900"
			/>
			<span class="text-xs font-medium text-surface-50 truncate flex-1">
				{selectedPal.display_name}
			</span>
		{:else}
			<Search size={14} class="text-surface-400" />
			<span class="text-xs text-surface-400 flex-1">{placeholder}</span>
		{/if}
		<ChevronDown size={14} class="text-surface-400 shrink-0" />
	</button>

	{#if open}
		<div
			bind:this={floatingEl}
			{@attach portal()}
			class="bg-surface-900 border border-surface-700 rounded-4 shadow-xl flex flex-col max-h-80 min-w-56"
			style="position: fixed; z-index: 99999;"
			role="listbox"
		>
			<div class="p-2 border-b border-surface-700">
				<input
					type="text"
					bind:value={query}
					placeholder="Search pals…"
					class="input text-xs"
					autocomplete="off"
				/>
			</div>
			<div class="overflow-y-auto flex-1">
				{#if filtered.length === 0}
					<p class="text-xs text-surface-400 p-3 text-center">No matches</p>
				{:else}
					{#each filtered as pal (pal.tribe)}
						<button
							type="button"
							class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs hover:bg-surface-800 transition-colors {pal.tribe === value ? 'bg-primary-500/15' : ''}"
							onclick={() => pick(pal)}
						>
							<img
								src={assetLoader.loadPalImage(pal.tribe)}
								alt={pal.display_name}
								class="w-5 h-5 object-contain rounded-2 bg-surface-900 shrink-0"
								loading="lazy"
							/>
							<span class="font-medium text-surface-50 truncate flex-1"
								>{pal.display_name}</span
							>
							<span class="text-[9px] text-surface-400 font-mono shrink-0"
								>R{pal.rarity ?? '-'}</span
							>
						</button>
					{/each}
				{/if}
			</div>
		</div>
	{/if}
</div>
