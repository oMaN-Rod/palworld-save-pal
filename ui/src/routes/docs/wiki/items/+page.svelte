<script lang="ts">
	import { itemsData } from '$lib/data';
	import WikiSearch from '$components/docs/WikiSearch.svelte';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { c } from '$lib/utils/commonTranslations';
	import { Rarity } from '$types';

	let search = $state('');
	let selectedKey = $state<string | null>(null);

	const allItems = $derived(
		Object.entries(itemsData.items)
			.filter(([, item]) => !item.details.disabled)
			.sort((a, b) => a[1].details.sort_id - b[1].details.sort_id)
	);

	const filteredItems = $derived(
		search
			? allItems.filter(
					([key, item]) =>
						item.info.localized_name.toLowerCase().includes(search.toLowerCase()) ||
						key.toLowerCase().includes(search.toLowerCase())
				)
			: allItems
	);

	const selectedItem = $derived(
		selectedKey ? itemsData.items[selectedKey] : null
	);

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
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="mb-3 flex items-center justify-between">
			<h1 class="text-lg font-bold">{c.item} Wiki</h1>
			<span class="text-xs text-surface-400">{filteredItems.length}</span>
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

	<div class="flex-1 overflow-y-auto rounded-lg border border-surface-800  p-5">
		{#if selectedItem && selectedKey}
			<div class="flex items-center gap-3">
				<img src={getItemIcon(selectedItem.details.icon)} alt="" class="h-12 w-12 object-contain" />
				<div>
					<h2 class="text-2xl font-bold">{selectedItem.info.localized_name}</h2>
					<span class="text-sm {rarityColor(selectedItem.details.rarity)}">{rarityLabel(selectedItem.details.rarity)}</span>
				</div>
			</div>

			<p class="mt-3 text-surface-300">{selectedItem.info.description}</p>

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
