<script lang="ts">
	import { itemsData } from '$lib/data';
	import { Rarity, type ItemContainerSlot } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$types/icons';
	import { assetLoader } from '$utils';
	import { Tooltip } from '$components/ui';

	let { items = $bindable([]) } = $props<{
		items: ItemContainerSlot[];
	}>();

	function getItemBackground(rarity: Rarity): string {
		switch (rarity) {
			case Rarity.Uncommon:
				return 'bg-linear-to-tl from-green-500/50';
			case Rarity.Rare:
				return 'bg-linear-to-tl from-blue-500/50';
			case Rarity.Epic:
				return 'bg-linear-to-tl from-purple-500/50';
			case Rarity.Legendary:
				return 'bg-linear-to-tl from-yellow-500/50';
			default:
				return '';
		}
	}

	function fixStupidTypos(key: string) {
		switch (key) {
			case 'Stonepit':
				return 'StonePit';
			case 'bone':
				return 'Bone';
			default:
				return key;
		}
	}
</script>

<div class="grid grid-cols-3 gap-2 space-y-2 2xl:grid-cols-4">
	{#each items as item}
		{@const itemData = itemsData.items[fixStupidTypos(item.static_id)]}
		{#if itemData}
			{@const itemIcon = assetLoader.loadImage(
				`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`
			)}
			<Tooltip label={`Slot ${item.slot_index + 1}`}>
				<div class="grid w-full grid-cols-[auto_1fr_auto] items-center gap-2">
					<div class={getItemBackground(itemData.details.rarity)}>
						<img
							src={itemIcon || staticIcons.unknownIcon}
							alt={itemData.info.localized_name}
							class="h-8 w-8"
						/>
					</div>
					<span>{itemData.info.localized_name}</span>
					<span class="bg-surface-800 rounded-sm p-2 font-bold">{item.count.toLocaleString()}</span>
				</div>
			</Tooltip>
		{/if}
	{/each}
</div>
