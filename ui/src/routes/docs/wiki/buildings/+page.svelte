<script lang="ts">
	import { buildingsData, itemsData } from '$lib/data';
	import { WikiSearch } from '$components/docs';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';

	let search = $state('');
	let selectedKey = $state<string | null>(null);

	const allBuildings = $derived(
		Object.entries(buildingsData.buildings).sort((a, b) =>
			a[1].localized_name.localeCompare(b[1].localized_name)
		)
	);

	const filteredBuildings = $derived(
		search
			? allBuildings.filter(
					([key, building]) =>
						building.localized_name.toLowerCase().includes(search.toLowerCase()) ||
						key.toLowerCase().includes(search.toLowerCase())
				)
			: allBuildings
	);

	const selectedBuilding = $derived(
		selectedKey ? buildingsData.buildings[selectedKey] : null
	);

	function getBuildingIcon(icon: string): string {
		if (!icon) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${icon}.webp`) as string;
	}
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="mb-3 flex items-center justify-between">
			<h1 class="text-lg font-bold">Buildings Wiki</h1>
			<span class="text-xs text-surface-400">{filteredBuildings.length}</span>
		</div>
		<div class="mb-3">
			<WikiSearch bind:value={search} />
		</div>
		<div class="flex-1 overflow-y-auto">
			{#each filteredBuildings as [key, building]}
				<button
					class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedKey === key
						? 'bg-surface-700 text-surface-50'
						: 'text-surface-300 hover:bg-surface-800'}"
					onclick={() => (selectedKey = key)}
				>
					<img src={getBuildingIcon(building.icon)} alt="" class="h-6 w-6 shrink-0 object-contain" />
					<span class="truncate font-medium">{building.localized_name}</span>
				</button>
			{/each}
		</div>
	</div>

	<div class="flex-1 overflow-y-auto rounded-lg border border-surface-800  p-5">
		{#if selectedBuilding && selectedKey}
			<div class="flex items-center gap-3">
				<img src={getBuildingIcon(selectedBuilding.icon)} alt="" class="h-12 w-12 object-contain" />
				<div>
					<h2 class="text-2xl font-bold">{selectedBuilding.localized_name}</h2>
					<span class="text-sm text-surface-400">{selectedBuilding.type_a} / {selectedBuilding.type_b}</span>
				</div>
			</div>

			<div>
				<p class="text-surface-300 mt-3">{selectedBuilding.description}</p>
			</div>

			<div class="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-3">
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Rank</span>
					<p class="text-sm">{selectedBuilding.rank}</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">HP</span>
					<p class="text-sm">{selectedBuilding.hp}</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Defense</span>
					<p class="text-sm">{selectedBuilding.defense}</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Work Amount</span>
					<p class="text-sm">{selectedBuilding.required_build_work_amount}</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Deterioration</span>
					<p class="text-sm">{selectedBuilding.deterioration_damage}</p>
				</div>
			</div>

			{#if selectedBuilding.materials && Object.keys(selectedBuilding.materials).length > 0}
				<div class="mt-5">
					<h3 class="mb-2 text-sm font-semibold text-surface-400">Materials</h3>
					<div class="flex flex-wrap gap-2">
						{#each Object.entries(selectedBuilding.materials) as [idx, mat]}
							{@const itemData = itemsData.getByKey(mat.id)!}
							{@const itemIcon = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`) as string}
							<span class="rounded-md bg-surface-900 px-3 py-1 text-sm">
								<img src={itemIcon} alt="" class="inline h-4 w-4 mr-1 object-contain" />
								{itemData?.info.localized_name || mat.id} <span class="font-semibold text-surface-400">x {mat.count}</span>
							</span>
						{/each}
					</div>
				</div>
			{/if}
		{:else}
			<div class="flex h-full items-center justify-center text-surface-500">
				<p>Select a building to view details</p>
			</div>
		{/if}
	</div>
</div>
