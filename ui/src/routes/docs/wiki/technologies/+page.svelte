<script lang="ts">
	import { buildingsData, itemsData, technologiesData } from '$lib/data';
	import WikiSearch from '$components/docs/WikiSearch.svelte';
	import * as m from '$i18n/messages';
	import { assetLoader } from '$utils';
	import { ASSET_DATA_PATH } from '$lib/constants';

	let search = $state('');
	let selectedKey = $state<string | null>(null);

	const allTechs = $derived(
		Object.entries(technologiesData.technologies).sort(
			(a, b) => (a[1].details.tier ?? 0) - (b[1].details.tier ?? 0)
		)
	);

	const filteredTechs = $derived(
		search
			? allTechs.filter(
					([key, tech]) =>
						tech.localized_name.toLowerCase().includes(search.toLowerCase()) ||
						key.toLowerCase().includes(search.toLowerCase())
				)
			: allTechs
	);

	const selectedTech = $derived(selectedKey ? technologiesData.technologies[selectedKey] : null);
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="mb-3 flex items-center justify-between">
			<h1 class="text-lg font-bold">{m.technology({ count: 2 })}</h1>
			<span class="text-surface-400 text-xs">{filteredTechs.length}</span>
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
					<div class="flex flex-wrap gap-2">
						{#each selectedTech.details.unlock_build_objects as obj}
							{@const buildingData = buildingsData.getByKey(obj)!} <!-- Assuming build_objects contains building data -->
							{@const buildingIcon = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${buildingData.icon}.webp`) as string}
							<span class="bg-surface-900 rounded-md px-3 py-1 text-sm flex items-center gap-2">
								<img src={buildingIcon} alt={buildingData.localized_name} class="h-4 w-4 object-contain" />
								{buildingData.localized_name || obj}
							</span>
						{/each}
					</div>
				</div>
			{/if}
			{#if selectedTech.details.unlock_item_recipes && selectedTech.details.unlock_item_recipes.length > 0}
				<div class="mt-5">
					<h3 class="text-surface-400 mb-2 text-sm font-semibold">Unlocks Item Recipes</h3>
					<div class="flex flex-wrap gap-2">
						{#each selectedTech.details.unlock_item_recipes as recipe}
							{@const itemData = itemsData.getByKey(recipe)!}
							{@const itemIcon = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`) as string}
							<span class="bg-surface-900 rounded-md px-3 py-1 text-sm">
								<img src={itemIcon} alt={itemData.info.localized_name || recipe} class="inline-block h-4 w-4 mr-1" />
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
