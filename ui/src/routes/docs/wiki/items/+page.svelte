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
		ArrowDown10
	} from 'lucide-svelte';
	import type { ValueChangeDetails } from '@zag-js/accordion';

	type SortBy = 'name' | 'price' | 'weight' | 'sort-id';
	type SortOrder = 'asc' | 'desc';

	let search = $state('');
	let selectedTypeFilter = $state('All');
	let selectedRarityFilter = $state('All');
	let sortBy: SortBy = $state('sort-id');
	let sortOrder: SortOrder = $state('asc');
	let filterExpand = $state(['']);

	const typeFilters = [
		'All',
		'Weapon',
		'Armor',
		'Accessory',
		'Material',
		'Consume',
		'Ammo',
		'Food',
		'Essential',
		'Glider'
	];
	const rarityFilters = [
		{ label: 'All', value: 'All', color: '' },
		{ label: 'Common', value: '0', color: 'text-surface-300' },
		{ label: 'Uncommon', value: '1', color: 'text-green-400' },
		{ label: 'Rare', value: '2', color: 'text-blue-400' },
		{ label: 'Epic', value: '3', color: 'text-purple-400' },
		{ label: 'Legend', value: '4', color: 'text-yellow-400' }
	];

	const typeFilterClass = (value: string) =>
		cn(
			'btn btn-sm px-2 py-1 text-xs rounded',
			selectedTypeFilter === value ? 'bg-secondary-500/25' : ''
		);
	const rarityFilterClass = (value: string) =>
		cn(
			'btn btn-sm px-2 py-1 text-xs rounded',
			selectedRarityFilter === value ? 'bg-secondary-500/25' : ''
		);
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
			case Rarity.Uncommon:
				return 'text-green-400';
			case Rarity.Rare:
				return 'text-blue-400';
			case Rarity.Epic:
				return 'text-purple-400';
			case Rarity.Legendary:
				return 'text-yellow-400';
			default:
				return 'text-surface-300';
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
</script>

<div class="flex h-full flex-col gap-4">
	
	<span class="text-surface-400 text-xs w-full text-end mb-2">{filteredItems.length}</span>

	<div class="flex flex-col gap-3 md:flex-row md:items-start">
		<div class="md:w-72">
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
							<legend class="text-surface-400 text-xs font-bold">Sort</legend>
							<div class="mt-1 grid grid-cols-3 gap-1">
								<button
									type="button"
									class={sortButtonClass('name')}
									onclick={() => toggleSort('name')}
									title="Name"
								>
									<NameSortIcon class="h-4 w-4" />
								</button>
								<button
									type="button"
									class={sortButtonClass('price')}
									onclick={() => toggleSort('price')}
									title="Price"
								>
									<PriceSortIcon class="h-4 w-4" />
								</button>
								<button
									type="button"
									class={sortButtonClass('weight')}
									onclick={() => toggleSort('weight')}
									title="Weight"
								>
									<WeightSortIcon class="h-4 w-4" />
								</button>
							</div>
						</div>
						<div class="mb-2">
							<legend class="text-surface-400 text-xs font-bold">Type</legend>
							<div class="mt-1 grid grid-cols-3 gap-1">
								{#each typeFilters as type (type)}
									<button
										type="button"
										class={typeFilterClass(type)}
										onclick={() => (selectedTypeFilter = type)}
									>
										{type}
									</button>
								{/each}
							</div>
						</div>
						<div>
							<legend class="text-surface-400 text-xs font-bold">Rarity</legend>
							<div class="mt-1 grid grid-cols-3 gap-1">
								{#each rarityFilters as rf (rf.value)}
									<button
										type="button"
										class="{rarityFilterClass(rf.value)} {rf.color}"
										onclick={() => (selectedRarityFilter = rf.value)}
									>
										{rf.label}
									</button>
								{/each}
							</div>
						</div>
					{/snippet}
				</Accordion.Item>
			</Accordion>
		</div>
		<div class="flex-1">
			<WikiSearch bind:value={search} />
		</div>
	</div>

	<div class="min-h-0 flex-1">
		<div class="table-wrap h-full overflow-y-auto">
			<table class="table caption-bottom">
				<thead class="bg-surface-950 sticky top-0 z-10">
					<tr>
						<th>Name</th>
						<th>Code Name</th>
						<th>Type</th>
						<th>Rarity</th>
						<th class="text-right">Weight</th>
						<th class="text-right">Price</th>
						<th class="text-right">Max Stack</th>
						<th>Description</th>
					</tr>
				</thead>
				<tbody class="[&>tr]:hover:preset-tonal-primary">
					{#each filteredItems as [key, item] (key)}
						<tr>
							<td>
								<div class="flex items-center gap-2">
									<img
										src={getItemIcon(item.details.icon)}
										alt=""
										class="h-10 w-10 shrink-0 object-contain"
									/>
									<span class="font-medium">{item.info.localized_name}</span>
								</div>
							</td>
							<td class="text-surface-400 font-mono text-xs">{key}</td>
							<td>{item.details.type_a}</td>
							<td class={rarityColor(item.details.rarity)}
								>{rarityLabel(item.details.rarity)}</td
							>
							<td class="text-right">{item.details.weight}</td>
							<td class="text-right">{item.details.price}</td>
							<td class="text-right">{item.details.max_stack_count}</td>
							<td class="text-surface-300 text-sm">{item.info.description}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</div>
</div>