<script lang="ts">
	import { buildingsData, itemsData } from '$lib/data';
	import { WikiSearch } from '$components/docs';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
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
	let selectedFilter = $state('All');
	let sortBy: SortBy = $state('name');
	let sortOrder: SortOrder = $state('asc');
	let filterExpand = $state(['']);

	type SortBy = 'name' | 'hp' | 'rank';
	type SortOrder = 'asc' | 'desc';

	const typeFilters = [
		'All',
		'Product',
		'Pal',
		'Storage',
		'Food',
		'Infrastructure',
		'Defense',
		'Furniture',
		'Other'
	];

	const filterClass = (value: string) =>
		cn(
			'btn btn-sm px-2 py-1 text-xs rounded',
			selectedFilter === value ? 'bg-secondary-500/25' : ''
		);
	const sortButtonClass = (value: SortBy) =>
		cn('btn', sortBy === value ? 'bg-secondary-500/25' : '');

	const NameSortIcon = $derived.by(() => {
		if (sortBy !== 'name') return ArrowDownAZ;
		return sortOrder === 'asc' ? ArrowDownAZ : ArrowDownZA;
	});
	const HpSortIcon = $derived.by(() => {
		if (sortBy !== 'hp') return ArrowDown01;
		return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
	});
	const RankSortIcon = $derived.by(() => {
		if (sortBy !== 'rank') return ArrowDown01;
		return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
	});

	function toggleSort(newSortBy: SortBy) {
		if (sortBy === newSortBy) {
			if (sortOrder === 'desc') {
				sortBy = 'name';
				sortOrder = 'asc';
			} else {
				sortOrder = 'desc';
			}
		} else {
			sortBy = newSortBy;
			sortOrder = 'asc';
		}
	}

	const allBuildings = $derived(
		Object.entries(buildingsData.buildings).filter(([, building]) => !building.disabled)
	);

	const filteredBuildings = $derived.by(() => {
		let result = allBuildings;

		if (selectedFilter !== 'All') {
			result = result.filter(([, building]) => building.type_a === selectedFilter);
		}

		if (search) {
			const q = search.toLowerCase();
			result = result.filter(
				([key, building]) =>
					building.localized_name.toLowerCase().includes(q) || key.toLowerCase().includes(q)
			);
		}

		result = [...result].sort((a, b) => {
			let cmp = 0;
			switch (sortBy) {
				case 'name':
					cmp = a[1].localized_name.localeCompare(b[1].localized_name);
					break;
				case 'hp':
					cmp = a[1].hp - b[1].hp;
					break;
				case 'rank':
					cmp = a[1].rank - b[1].rank;
					break;
			}
			return sortOrder === 'asc' ? cmp : -cmp;
		});

		return result;
	});

	const selectedBuilding = $derived(selectedKey ? buildingsData.buildings[selectedKey] : null);

	function getBuildingIcon(icon: string): string {
		if (!icon) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${icon}.webp`) as string;
	}
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<span class="text-surface-400 mb-2 w-full text-end text-xs">{filteredBuildings.length}</span>
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
									class={sortButtonClass('hp')}
									onclick={() => toggleSort('hp')}
									title="HP"
								>
									<HpSortIcon class="h-4 w-4" />
								</button>
								<button
									type="button"
									class={sortButtonClass('rank')}
									onclick={() => toggleSort('rank')}
									title="Rank"
								>
									<RankSortIcon class="h-4 w-4" />
								</button>
							</div>
						</div>
						<div>
							<legend class="text-surface-400 text-xs font-bold">Type</legend>
							<div class="mt-1 grid grid-cols-3 gap-1">
								{#each typeFilters as type}
									<button
										type="button"
										class={filterClass(type)}
										onclick={() => (selectedFilter = type)}
									>
										{type === 'All' ? 'All' : type}
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
			{#each filteredBuildings as [key, building]}
				<button
					class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedKey ===
					key
						? 'bg-surface-700 text-surface-50'
						: 'text-surface-300 hover:bg-surface-800'}"
					onclick={() => (selectedKey = key)}
				>
					<img
						src={getBuildingIcon(building.icon)}
						alt=""
						class="h-6 w-6 shrink-0 object-contain"
					/>
					<span class="truncate font-medium">{building.localized_name}</span>
				</button>
			{/each}
		</div>
	</div>

	<div class="border-surface-800 flex-1 overflow-y-auto rounded-lg border p-5">
		{#if selectedBuilding && selectedKey}
			<div class="flex items-center gap-3">
				<img src={getBuildingIcon(selectedBuilding.icon)} alt="" class="h-12 w-12 object-contain" />
				<div>
					<h2 class="text-2xl font-bold">{selectedBuilding.localized_name}</h2>
					<span class="text-surface-400 text-sm"
						>{selectedBuilding.type_a} / {selectedBuilding.type_b}</span
					>
				</div>
			</div>

			<div>
				<p class="text-surface-300 mt-3">{selectedBuilding.description}</p>
			</div>

			<div class="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-3">
				<div class="bg-surface-900 rounded-md p-3">
					<span class="text-surface-500 text-xs">Rank</span>
					<p class="text-sm">{selectedBuilding.rank}</p>
				</div>
				<div class="bg-surface-900 rounded-md p-3">
					<span class="text-surface-500 text-xs">HP</span>
					<p class="text-sm">{selectedBuilding.hp}</p>
				</div>
				<div class="bg-surface-900 rounded-md p-3">
					<span class="text-surface-500 text-xs">Defense</span>
					<p class="text-sm">{selectedBuilding.defense}</p>
				</div>
				<div class="bg-surface-900 rounded-md p-3">
					<span class="text-surface-500 text-xs">Work Amount</span>
					<p class="text-sm">{selectedBuilding.required_build_work_amount}</p>
				</div>
				<div class="bg-surface-900 rounded-md p-3">
					<span class="text-surface-500 text-xs">Deterioration</span>
					<p class="text-sm">{selectedBuilding.deterioration_damage}</p>
				</div>
			</div>

			{#if selectedBuilding.materials && selectedBuilding.materials.length > 0}
				<div class="mt-5">
					<h3 class="text-surface-400 mb-2 text-sm font-semibold">Materials</h3>
					<div class="flex flex-wrap gap-2">
						{#each selectedBuilding.materials as mat}
							{@const itemData = itemsData.getByKey(mat.id)!}
							{@const itemIcon = assetLoader.loadImage(
								`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`
							) as string}
							<span class="bg-surface-900 rounded-md px-3 py-1 text-sm">
								<img src={itemIcon} alt="" class="mr-1 inline h-4 w-4 object-contain" />
								{itemData?.info.localized_name || mat.id}
								<span class="text-surface-400 font-semibold">x {mat.count}</span>
							</span>
						{/each}
					</div>
				</div>
			{/if}
		{:else}
			<div class="text-surface-500 flex h-full items-center justify-center">
				<p>Select a building to view details</p>
			</div>
		{/if}
	</div>
</div>
