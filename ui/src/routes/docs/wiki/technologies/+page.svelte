<script lang="ts">
	import { buildingsData, itemsData, technologiesData } from '$lib/data';
	import { WikiSearch } from '$components/docs';
	import * as m from '$i18n/messages';
	import { assetLoader } from '$utils';
	import { ASSET_DATA_PATH } from '$lib/constants';
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
	let sortBy: SortBy = $state('tier');
	let sortOrder: SortOrder = $state('asc');
	let filterExpand = $state(['']);

	type SortBy = 'name' | 'level-cap' | 'cost' | 'tier';
	type SortOrder = 'asc' | 'desc';

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
	const LevelCapSortIcon = $derived.by(() => {
		if (sortBy !== 'level-cap') return ArrowDown01;
		return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
	});
	const CostSortIcon = $derived.by(() => {
		if (sortBy !== 'cost') return ArrowDown01;
		return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
	});
	const TierSortIcon = $derived.by(() => {
		if (sortBy !== 'tier') return ArrowDown01;
		return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
	});

	function toggleSort(newSortBy: SortBy) {
		if (sortBy === newSortBy) {
			if (sortOrder === 'desc') {
				sortBy = 'tier';
				sortOrder = 'asc';
			} else {
				sortOrder = 'desc';
			}
		} else {
			sortBy = newSortBy;
			sortOrder = 'asc';
		}
	}

	const allTechs = $derived(
		Object.entries(technologiesData.technologies).sort((a, b) =>
			(a[1].details.level_cap ?? 0) - (b[1].details.level_cap ?? 0)
		)
	);

	const filteredTechs = $derived.by(() => {
		let result = allTechs;

		if (selectedFilter === 'boss') {
			result = result.filter(([, tech]) => tech.details.is_boss_technology);
		} else if (selectedFilter === 'normal') {
			result = result.filter(([, tech]) => !tech.details.is_boss_technology);
		}

		if (search) {
			const q = search.toLowerCase();
			result = result.filter(
				([key, tech]) =>
					tech.localized_name.toLowerCase().includes(q) || key.toLowerCase().includes(q)
			);
		}

		result = [...result].sort((a, b) => {
			let cmp = 0;
			switch (sortBy) {
				case 'name':
					cmp = a[1].localized_name.localeCompare(b[1].localized_name);
					break;
				case 'level-cap':
					cmp = (a[1].details.level_cap ?? 0) - (b[1].details.level_cap ?? 0);
					break;
				case 'cost':
					cmp = (a[1].details.cost ?? 0) - (b[1].details.cost ?? 0);
					break;
				case 'tier':
					cmp = (a[1].details.tier ?? 0) - (b[1].details.tier ?? 0);
					break;
			}
			return sortOrder === 'asc' ? cmp : -cmp;
		});

		return result;
	});

	const selectedTech = $derived(selectedKey ? technologiesData.technologies[selectedKey] : null);
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<span class="text-surface-400 mb-2 w-full text-end text-xs">{filteredTechs.length}</span>
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
							<div class="mt-1 grid grid-cols-4 gap-1">
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
									class={sortButtonClass('tier')}
									onclick={() => toggleSort('tier')}
									title="Tier"
								>
									<TierSortIcon class="h-4 w-4" />
								</button>
								<button
									type="button"
									class={sortButtonClass('level-cap')}
									onclick={() => toggleSort('level-cap')}
									title="Level Cap"
								>
									<LevelCapSortIcon class="h-4 w-4" />
								</button>
								<button
									type="button"
									class={sortButtonClass('cost')}
									onclick={() => toggleSort('cost')}
									title="Cost"
								>
									<CostSortIcon class="h-4 w-4" />
								</button>
							</div>
						</div>
						<div>
							<legend class="text-surface-400 text-xs font-bold">Filter</legend>
							<div class="mt-1 grid grid-cols-3 gap-1">
								<button
									type="button"
									class={filterClass('All')}
									onclick={() => (selectedFilter = 'All')}>All</button
								>
								<button
									type="button"
									class={filterClass('normal')}
									onclick={() => (selectedFilter = 'normal')}>Normal</button
								>
								<button
									type="button"
									class={filterClass('boss')}
									onclick={() => (selectedFilter = 'boss')}>Ancient</button
								>
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
			{#each filteredTechs as [key, tech]}
				{@const technologyIcon = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/${tech.details.icon}.webp`
				) as string}
				<button
					class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedKey ===
					key
						? 'bg-surface-700 text-surface-50'
						: 'text-surface-300 hover:bg-surface-800'}"
					onclick={() => (selectedKey = key)}
				>
					<img src={technologyIcon} alt={tech.localized_name} class="h-4 w-4 object-contain" />
					<span class="truncate font-medium">{tech.localized_name}</span>
					{#if tech.details.tier}
						<span class="text-surface-500 ml-auto text-xs">T{tech.details.tier}</span>
					{/if}
					{#if tech.details.level_cap}
						<span class="text-surface-500 ml-auto text-xs">Lv {tech.details.level_cap}</span>
					{/if}
				</button>
			{/each}
		</div>
	</div>

	<div class="border-surface-800 flex-1 overflow-y-auto rounded-lg border p-5">
		{#if selectedTech && selectedKey}
			<div class="flex items-center justify-between">
				<h2 class="text-2xl font-bold">{selectedTech.localized_name}</h2>
				{#if selectedTech.details.tier}
					<span class="text-surface-400 text-sm">Tier {selectedTech.details.tier}</span>
				{/if}
			</div>
			<span class="text-surface-400 text-sm">{selectedTech.details.category}</span>

			<p class="text-surface-300 mt-3">{selectedTech.description}</p>

			<div class="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-3">
				{#if selectedTech.details.cost}
					<div class="bg-surface-900 rounded-md p-3">
						<span class="text-surface-500 text-xs">Cost</span>
						<p class="text-lg font-semibold">{selectedTech.details.cost}</p>
					</div>
				{/if}
				{#if selectedTech.details.level_cap}
					<div class="bg-surface-900 rounded-md p-3">
						<span class="text-surface-500 text-xs">Level Cap</span>
						<p class="text-lg font-semibold">{selectedTech.details.level_cap}</p>
					</div>
				{/if}
				{#if selectedTech.details.work_amount}
					<div class="bg-surface-900 rounded-md p-3">
						<span class="text-surface-500 text-xs">Work Amount</span>
						<p class="text-sm">{selectedTech.details.work_amount}</p>
					</div>
				{/if}
			</div>

			{#if selectedTech.details.materials && Object.keys(selectedTech.details.materials).length > 0}
				<div class="mt-5">
					<h3 class="text-surface-400 mb-2 text-sm font-semibold">Materials</h3>
					<div class="flex flex-wrap gap-2">
						{#each Object.entries(selectedTech.details.materials) as [mat, count]}
							<span class="bg-surface-900 rounded-md px-3 py-1 text-sm">
								{mat}: <span class="text-primary-400 font-semibold">{count}</span>
							</span>
						{/each}
					</div>
				</div>
			{/if}

			{#if selectedTech.details.unlock_build_objects && selectedTech.details.unlock_build_objects.length > 0}
				<div class="mt-5">
					<h3 class="text-surface-400 mb-2 text-sm font-semibold">Unlocks Buildings</h3>
					<div class="grid grid-cols-3 2xl:grid-cols-5 gap-2">
						{#each selectedTech.details.unlock_build_objects as obj}
							{@const buildingData = buildingsData.getByKey(obj)!}
							{@const buildingIcon = assetLoader.loadImage(
								`${ASSET_DATA_PATH}/img/${buildingData.icon}.webp`
							) as string}
							<span class="bg-surface-900 flex items-center gap-2 rounded-md px-3 py-1 text-sm">
								<img
									src={buildingIcon}
									alt={buildingData.localized_name}
									class="h-16 w-16 object-contain"
								/>
								{buildingData.localized_name || obj}
							</span>
						{/each}
					</div>
				</div>
			{/if}
			{#if selectedTech.details.unlock_item_recipes && selectedTech.details.unlock_item_recipes.length > 0}
				<div class="mt-5">
					<h3 class="text-surface-400 mb-2 text-sm font-semibold">Unlocks Item Recipes</h3>
					<div class="grid grid-cols-3 2xl:grid-cols-5 gap-2">
						{#each selectedTech.details.unlock_item_recipes as recipe}
							{@const itemData = itemsData.getByKey(recipe)!}
							{@const itemIcon = assetLoader.loadImage(
								`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`
							) as string}
							<span class="bg-surface-900 rounded-md px-3 py-1 text-sm">
								<img
									src={itemIcon}
									alt={itemData.info.localized_name || recipe}
									class="mr-1 inline-block h-16 w-16"
								/>
								{itemData.info.localized_name || recipe}
							</span>
						{/each}
					</div>
				</div>
			{/if}
		{:else}
			<div class="text-surface-500 flex h-full items-center justify-center">
				<p>Select a technology to view details</p>
			</div>
		{/if}
	</div>
</div>
