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
	import * as m from '$i18n/messages';
	import Search from '@lucide/svelte/icons/search';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import LayoutGrid from '@lucide/svelte/icons/layout-grid';
	import List from '@lucide/svelte/icons/list';
	import type { BreedablePal } from '$lib/breeding/types';

	let {
		value = null,
		placeholder = m.breeding_select_pal(),
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
	let viewMode = $state<'grid' | 'list'>('grid');
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
		class="input hover:border-primary-500/50 flex h-11 cursor-pointer items-center gap-2 text-left transition-colors"
		onclick={() => (open = !open)}
		aria-haspopup="listbox"
		aria-expanded={open}
	>
		{#if selectedPal}
			<img
				src={assetLoader.loadMenuImage(selectedPal.tribe)}
				alt={selectedPal.display_name}
				class="bg-surface-900 h-7 w-7 rounded-sm object-contain"
			/>
			<span class="text-surface-50 flex-1 truncate text-sm font-medium">
				{selectedPal.display_name}
			</span>
		{:else}
			<Search size={16} class="text-surface-400" />
			<span class="text-surface-400 flex-1 text-sm">{placeholder}</span>
		{/if}
		<ChevronDown size={16} class="text-surface-400 shrink-0" />
	</button>

	{#if open}
		<div
			bind:this={floatingEl}
			{@attach portal()}
			class="bg-surface-900 border-surface-700 flex max-h-96 min-w-72 flex-col rounded-md border shadow-xl"
			style="position: fixed; z-index: 99999;"
			role="listbox"
		>
			<div class="border-surface-700 flex items-center gap-2 border-b p-2">
				<input
					type="text"
					bind:value={query}
					placeholder={m.breeding_search_pals()}
					class="input flex-1 text-sm"
					autocomplete="off"
				/>
				<div class="flex shrink-0 gap-0.5">
					<button
						type="button"
						class="rounded-sm p-1.5 transition-colors {viewMode === 'grid'
							? 'bg-primary-500/15 text-primary-300'
							: 'text-surface-400 hover:text-surface-200'}"
						onclick={() => (viewMode = 'grid')}
						title="Grid"
					>
						<LayoutGrid size={15} />
					</button>
					<button
						type="button"
						class="rounded-sm p-1.5 transition-colors {viewMode === 'list'
							? 'bg-primary-500/15 text-primary-300'
							: 'text-surface-400 hover:text-surface-200'}"
						onclick={() => (viewMode = 'list')}
						title="List"
					>
						<List size={15} />
					</button>
				</div>
			</div>
			<div class="flex-1 overflow-y-auto">
				{#if filtered.length === 0}
					<p class="text-surface-400 p-3 text-center text-sm">{m.breeding_no_matches()}</p>
				{:else if viewMode === 'grid'}
					<div
						class="grid gap-1 p-2"
						style="grid-template-columns: repeat(auto-fill, minmax(72px, 1fr));"
					>
						{#each filtered as pal (pal.tribe)}
							<button
								type="button"
								class="hover:bg-surface-800 flex flex-col items-center gap-1 rounded-sm p-1.5 transition-colors {pal.tribe ===
								value
									? 'bg-primary-500/15'
									: ''}"
								onclick={() => pick(pal)}
							>
								<img
									src={assetLoader.loadMenuImage(pal.tribe)}
									alt={pal.display_name}
									class="bg-surface-900 h-10 w-10 rounded-sm object-contain"
									loading="lazy"
								/>
								<span class="text-surface-200 w-full truncate text-center text-xs"
									>{pal.display_name}</span
								>
							</button>
						{/each}
					</div>
				{:else}
					{#each filtered as pal (pal.tribe)}
						<button
							type="button"
							class="hover:bg-surface-800 flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors {pal.tribe ===
							value
								? 'bg-primary-500/15'
								: ''}"
							onclick={() => pick(pal)}
						>
							<img
								src={assetLoader.loadMenuImage(pal.tribe)}
								alt={pal.display_name}
								class="bg-surface-900 h-7 w-7 shrink-0 rounded-sm object-contain"
								loading="lazy"
							/>
							<span class="text-surface-50 flex-1 truncate font-medium">{pal.display_name}</span>
							<span class="text-surface-400 shrink-0 font-mono text-[10px]"
								>R{pal.rarity ?? '-'}</span
							>
						</button>
					{/each}
				{/if}
			</div>
		</div>
	{/if}
</div>
