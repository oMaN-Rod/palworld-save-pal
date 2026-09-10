<script lang="ts">
	// Dropdown is portaled to <body> and positioned via Floating UI: plain `absolute z-50` fails
	// because the trigger sits inside stacking contexts (.card's backdrop-filter, panels' overflow)
	// that trap the z-index and clip the menu.
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { assetLoader } from '$lib/utils/assetLoader';
	import { computePosition, flip, shift, offset, autoUpdate } from '@floating-ui/dom';
	import { portal } from '$utils';
	import * as m from '$i18n/messages';
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
		class="input flex h-11 items-center gap-2 text-left cursor-pointer hover:border-primary-500/50 transition-colors"
		onclick={() => (open = !open)}
		aria-haspopup="listbox"
		aria-expanded={open}
	>
		{#if selectedPal}
			<img
				src={assetLoader.loadMenuImage(selectedPal.tribe)}
				alt={selectedPal.display_name}
				class="w-7 h-7 object-contain rounded-sm bg-surface-900"
			/>
			<span class="text-sm font-medium text-surface-50 truncate flex-1">
				{selectedPal.display_name}
			</span>
		{:else}
			<Icon icon="tabler:search" size={16} class="text-surface-400" />
			<span class="text-sm text-surface-400 flex-1">{placeholder}</span>
		{/if}
		<Icon icon="tabler:chevron-down" size={16} class="text-surface-400 shrink-0" />
	</button>

	{#if open}
		<div
			bind:this={floatingEl}
			{@attach portal()}
			class="bg-surface-900 border border-surface-700 rounded-md shadow-xl flex flex-col max-h-96 min-w-72"
			style="position: fixed; z-index: 99999;"
			role="listbox"
		>
			<div class="flex items-center gap-2 border-b border-surface-700 p-2">
				<input
					type="text"
					bind:value={query}
					placeholder={m.breeding_search_pals()}
					class="input text-sm flex-1"
					autocomplete="off"
				/>
				<div class="flex gap-0.5 shrink-0">
					<button
						type="button"
						class="rounded-sm p-1.5 transition-colors {viewMode === 'grid'
							? 'bg-primary-500/15 text-primary-300'
							: 'text-surface-400 hover:text-surface-200'}"
						onclick={() => (viewMode = 'grid')}
						title="Grid"
					>
						<Icon icon="tabler:layout-grid" size={15} />
					</button>
					<button
						type="button"
						class="rounded-sm p-1.5 transition-colors {viewMode === 'list'
							? 'bg-primary-500/15 text-primary-300'
							: 'text-surface-400 hover:text-surface-200'}"
						onclick={() => (viewMode = 'list')}
						title="List"
					>
						<Icon icon="tabler:list" size={15} />
					</button>
				</div>
			</div>
			<div class="overflow-y-auto flex-1">
				{#if filtered.length === 0}
					<p class="text-sm text-surface-400 p-3 text-center">{m.breeding_no_matches()}</p>
				{:else if viewMode === 'grid'}
					<div class="grid gap-1 p-2" style="grid-template-columns: repeat(auto-fill, minmax(72px, 1fr));">
						{#each filtered as pal (pal.tribe)}
							<button
								type="button"
								class="flex flex-col items-center gap-1 rounded-sm p-1.5 transition-colors hover:bg-surface-800 {pal.tribe ===
								value
									? 'bg-primary-500/15'
									: ''}"
								onclick={() => pick(pal)}
							>
								<img
									src={assetLoader.loadMenuImage(pal.tribe)}
									alt={pal.display_name}
									class="w-10 h-10 object-contain rounded-sm bg-surface-900"
									loading="lazy"
								/>
								<span class="text-xs text-surface-200 truncate w-full text-center"
									>{pal.display_name}</span
								>
							</button>
						{/each}
					</div>
				{:else}
					{#each filtered as pal (pal.tribe)}
						<button
							type="button"
							class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-surface-800 transition-colors {pal.tribe === value ? 'bg-primary-500/15' : ''}"
							onclick={() => pick(pal)}
						>
							<img
								src={assetLoader.loadMenuImage(pal.tribe)}
								alt={pal.display_name}
								class="w-7 h-7 object-contain rounded-sm bg-surface-900 shrink-0"
								loading="lazy"
							/>
							<span class="font-medium text-surface-50 truncate flex-1"
								>{pal.display_name}</span
							>
							<span class="text-[10px] text-surface-400 font-mono shrink-0"
								>R{pal.rarity ?? '-'}</span
							>
						</button>
					{/each}
				{/if}
			</div>
		</div>
	{/if}
</div>
