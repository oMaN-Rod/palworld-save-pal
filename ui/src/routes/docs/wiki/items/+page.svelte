<script lang="ts">
	import { itemsData } from '$lib/data';
	import { WikiSearch } from '$components/docs';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { c } from '$lib/utils/commonTranslations';
	import { Rarity } from '$types';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { cn } from '$theme';
	import {
		SlidersHorizontal,
		ArrowDownAZ,
		ArrowDownZA,
		ArrowDown01,
		ArrowDown10,
		GalleryVerticalEnd
	} from 'lucide-svelte';
	import type { ValueChangeDetails } from '@zag-js/accordion';

	let search = $state('');
	let selectedKey = $state<string | null>(null);
	let selectedTypeFilter = $state('All');
	let selectedRarityFilter = $state('All');
	let sortBy: SortBy = $state('sort-id');
	let sortOrder: SortOrder = $state('asc');
	let filterExpand = $state(['']);

	type SortBy = 'name' | 'price' | 'weight' | 'sort-id';
	type SortOrder = 'asc' | 'desc';

	const typeFilters = ['All', 'Weapon', 'Armor', 'Accessory', 'Material', 'Consume', 'Ammo', 'Food', 'Essential', 'Glider'];
	const rarityFilters = [
		{ label: 'All', value: 'All', color: '' },
		{ label: 'Common', value: '0', color: 'text-surface-300' },
		{ label: 'Uncommon', value: '1', color: 'text-green-400' },
		{ label: 'Rare', value: '2', color: 'text-blue-400' },
		{ label: 'Epic', value: '3', color: 'text-purple-400' },
		{ label: 'Legend', value: '4', color: 'text-yellow-400' }
	];

	const typeFilterClass = (value: string) =>
		cn('btn btn-sm px-2 py-1 text-xs rounded', selectedTypeFilter === value ? 'bg-secondary-500/25' : '');
	const rarityFilterClass = (value: string) =>
		cn('btn btn-sm px-2 py-1 text-xs rounded', selectedRarityFilter === value ? 'bg-secondary-500/25' : '');
	const sortButtonClass = (value: SortBy) =>
		cn('btn', sortBy === value ? 'bg-secondary-500/25' : '');

	const NameSortIcon = $derived.by(() => {
		if (sortBy !== 'name') return ArrowDownAZ;
		return sortOrder === 'asc' ? ArrowDownAZ : ArrowDownZA;
	});
	const PriceSortIcon = $derived.by(() => {
		if (sortBy !== 'price') return ArrowDown01;
		return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
	});
	const WeightSortIcon = $derived.by(() => {
		if (sortBy !== 'weight') return ArrowDown01;
		return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
	});

	function toggleSort(newSortBy: SortBy) {
		if (sortBy === newSortBy) {
			if (sortOrder === 'desc') {
				sortBy = 'sort-id';
				sortOrder = 'asc';
			} else {
				sortOrder = 'desc';
			}
		} else {
			sortBy = newSortBy;
			sortOrder = 'asc';
		}
	}

	function getItemIcon(icon: string): string {
		if (!icon) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${icon}.webp`) as string;
	}

	function rarityColor(rarity: Rarity): string {
		switch (rarity) {
			case Rarity.Uncommon: return 'text-green-400';
			case Rarity.Rare: return 'text-blue-400';
			case Rarity.Epic: return 'text-purple-400';
			case Rarity.Legendary: return 'text-yellow-400';
			default: return 'text-surface-300';
		}
	}

	function rarityLabel(rarity: Rarity): string {
		return Rarity[rarity] || 'Common';
	}

	const allItems = $derived(
		Object.entries(itemsData.items).filter(([, item]) => !item.details.disabled)
	);

	const filteredItems = $derived.by(() => {
		let result = allItems;

		if (selectedTypeFilter !== 'All') {
			result = result.filter(([, item]) => item.details.type_a === selectedTypeFilter);
		}

		if (selectedRarityFilter !== 'All') {
			const rarityNum = parseInt(selectedRarityFilter);
			result = result.filter(([, item]) => item.details.rarity === rarityNum);
		}

		if (search) {
			const q = search.toLowerCase();
			result = result.filter(
				([key, item]) =>
					item.info.localized_name.toLowerCase().includes(q) || key.toLowerCase().includes(q)
			);
		}

		result = [...result].sort((a, b) => {
			let cmp = 0;
			switch (sortBy) {
				case 'name':
					cmp = a[1].info.localized_name.localeCompare(b[1].info.localized_name);
					break;
				case 'price':
					cmp = a[1].details.price - b[1].details.price;
					break;
				case 'weight':
					cmp = a[1].details.weight - b[1].details.weight;
					break;
				case 'sort-id':
					cmp = a[1].details.sort_id - b[1].details.sort_id;
					break;
			}
			return sortOrder === 'asc' ? cmp : -cmp;
		});

		return result;
	});

	const selectedItem = $derived(selectedKey ? itemsData.items[selectedKey] : null);
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="mb-3 flex items-center justify-between">
			<h1 class="text-lg font-bold">{c.item} Wiki</h1>
			<span class="text-xs text-surface-400">{filteredItems.length}</span>
		</div>
		<div class="mb-3">
			<Accordion
				value={filterExpand}
				onValueChange={(e: ValueChangeDetails) => (filterExpand = e.value)}
				collapsible
			>
				<Accordion.Item
					value="filter"
					base="rounded-sm bg-surface-900"
					controlHover="hover:bg-secondary-500/25"
				>
					{#snippet lead()}<SlidersHorizontal class="h-4 w-4" />{/snippet}
					{#snippet control()}<span class="text-sm font-bold">Filter & Sort</span>{/snippet}
					{#snippet panel()}
						<div class="mb-2">
							<legend class="text-xs font-bold text-surface-400">Sort</legend>
							<div class="mt-1 grid grid-cols-3 gap-1">
								<button type="button" class={sortButtonClass('name')} onclick={() => toggleSort('name')} title="Name">
									<NameSortIcon class="h-4 w-4" />
								</button>
								<button type="button" class={sortButtonClass('price')} onclick={() => toggleSort('price')} title="Price">
									<PriceSortIcon class="h-4 w-4" />
								</button>
								<button type="button" class={sortButtonClass('weight')} onclick={() => toggleSort('weight')} title="Weight">
									<WeightSortIcon class="h-4 w-4" />
								</button>
							</div>
						</div>
						<div class="mb-2">
							<legend class="text-xs font-bold text-surface-400">Type</legend>
							<div class="mt-1 grid grid-cols-3 gap-1">
								{#each typeFilters as type}
									<button type="button" class={typeFilterClass(type)} onclick={() => (selectedTypeFilter = type)}>
										{type}
									</button>
								{/each}
							</div>
						</div>
						<div>
							<legend class="text-xs font-bold text-surface-400">Rarity</legend>
							<div class="mt-1 grid grid-cols-3 gap-1">
								{#each rarityFilters as rf}
									<button type="button" class="{rarityFilterClass(rf.value)} {rf.color}" onclick={() => (selectedRarityFilter = rf.value)}>
										{rf.label}
									</button>
								{/each}
							</div>
						</div>
					{/snippet}
				</Accordion.Item>
			</Accordion>
		</div>
		<div class="mb-3">
			<WikiSearch bind:value={search} />
		</div>
		<div class="flex-1 overflow-y-auto">
			{#each filteredItems as [key, item]}
				<button
					class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedKey === key
						? 'bg-surface-700 text-surface-50'
						: 'text-surface-300 hover:bg-surface-800'}"
					onclick={() => (selectedKey = key)}
				>
					<img src={getItemIcon(item.details.icon)} alt="" class="h-6 w-6 shrink-0 object-contain" />
					<span class="truncate font-medium">{item.info.localized_name}</span>
					<span class="ml-auto text-xs {rarityColor(item.details.rarity)}">{rarityLabel(item.details.rarity)}</span>
				</button>
			{/each}
		</div>
	</div>

	<div class="flex-1 overflow-y-auto rounded-lg border border-surface-800 p-5">
		{#if selectedItem && selectedKey}
			<div class="flex items-center gap-3">
				<img src={getItemIcon(selectedItem.details.icon)} alt="" class="h-12 w-12 object-contain" />
				<div>
					<h2 class="text-2xl font-bold">{selectedItem.info.localized_name}</h2>
					<span class="text-sm {rarityColor(selectedItem.details.rarity)}">{rarityLabel(selectedItem.details.rarity)}</span>
				</div>
			</div>

			<p class="text-surface-300 mt-3">{selectedItem.info.description}</p>

			<div class="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-3">
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Type</span>
					<p class="text-sm">{selectedItem.details.type_a}</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Group</span>
					<p class="text-sm">{selectedItem.details.group}</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Weight</span>
					<p class="text-sm">{selectedItem.details.weight}</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Price</span>
					<p class="text-sm">{selectedItem.details.price}</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Max Stack</span>
					<p class="text-sm">{selectedItem.details.max_stack_count}</p>
				</div>
				{#if selectedItem.details.damage}
					<div class="rounded-md bg-surface-900 p-3">
						<span class="text-xs text-surface-500">Damage</span>
						<p class="text-sm">{selectedItem.details.damage}</p>
					</div>
				{/if}
				{#if selectedItem.details.dynamic}
					<div class="rounded-md bg-surface-900 p-3">
						<span class="text-xs text-surface-500">Durability</span>
						<p class="text-sm">{selectedItem.details.dynamic.durability}</p>
					</div>
				{/if}
			</div>
		{:else}
			<div class="flex h-full items-center justify-center text-surface-500">
				<p>Select an item to view details</p>
			</div>
		{/if}
	</div>
</div>
