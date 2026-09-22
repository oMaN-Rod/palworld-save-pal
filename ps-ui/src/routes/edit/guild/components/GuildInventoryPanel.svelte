<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Input, List } from '$components/ui';
	import { buildingsData, itemsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';
	import { Rarity } from '$types';
	import * as m from '$i18n/messages';

	import { IGNORED_CONTAINER_KEYS, type GuildInventoryItem } from '../guildStorage';

	interface Props {
		items: GuildInventoryItem[];
		searchQuery: string;
		onSelect: (staticId: string) => void;
		onReset: () => void;
	}

	let { items, searchQuery = $bindable(), onSelect, onReset }: Props = $props();

	function itemBackground(rarity: Rarity): string {
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
</script>

<div class="flex items-center">
	<Input bind:value={searchQuery} placeholder={m.search_entity({ entity: m.inventory() })} />
	<Button id="guild-inventory-reset" variant="ghost" onclick={onReset}>
		<Icon icon="tabler:rotate" class="h-6 w-6" />
	</Button>
</div>
<List
	{items}
	baseClass="w-full"
	listClass="h-[calc(100vh-var(--titlebar-h)-350px)]"
	canSelect={false}
	idKey="static_id"
	headerClass="grid w-full grid-cols-[auto_1fr_auto] gap-2 rounded-sm"
	onselect={(item) => onSelect(item.static_id)}
	multiple={false}
>
	{#snippet listHeader()}
		<div class="h-8 w-8"></div>
		<span class="font-bold">{m.inventory()}</span>
		<span class="font-bold">{m.total()}</span>
	{/snippet}
	{#snippet listItem(item)}
		{@const itemData = itemsData.getByKey(item.static_id)}
		{#if itemData}
			{@const itemIcon = assetLoader.loadImage(
				`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`
			)}
			<div class="grid w-full grid-cols-[auto_1fr_auto] gap-2">
				<div class={itemBackground(itemData.details.rarity)}>
					<img
						src={itemIcon || staticIcons.unknownIcon}
						alt={itemData.info.localized_name}
						class="h-8 w-8"
					/>
				</div>
				<span class="truncate">{itemData.info.localized_name}</span>
				<span>{item.total_count.toLocaleString()}</span>
			</div>
		{:else}
			<div class="grid w-full grid-cols-[auto_1fr_auto] gap-2">
				<img src={staticIcons.unknownIcon} alt={item.static_id} class="h-8 w-8" />
				<span class="truncate">{item.static_id}</span>
				<span>{item.total_count.toLocaleString()}</span>
			</div>
		{/if}
	{/snippet}
	{#snippet listItemPopup(item)}
		{@const itemData = itemsData.getByKey(item.static_id)}
		{#if itemData}
			<div class="flex flex-col">
				<span class="font-bold">{itemData.info.localized_name}</span>
				<span class="text-sm">{itemData.info.description}</span>
				<hr class="border-surface-500 my-2" />
				<span class="font-bold">{m.total_count({ count: item.total_count })}</span>
				{#each Object.entries(item.containers) as [containerId, count]}
					{@const building = buildingsData.getByKey(containerId)}
					{#if building}
						{@const buildingIcon = assetLoader.loadImage(
							`${ASSET_DATA_PATH}/img/${building.icon}.webp`
						)}
						<div class="grid w-full min-w-0 grid-cols-[auto_1fr_auto] gap-2">
							<img
								src={buildingIcon || staticIcons.unknownIcon}
								alt={building.localized_name}
								class="h-8 w-8 shrink-0"
							/>
							<span class="truncate">{building.localized_name}</span>
							<span>{count.toLocaleString()}</span>
						</div>
					{:else if !IGNORED_CONTAINER_KEYS.some((key) => containerId.includes(key))}
						<div class="grid w-full grid-cols-2 gap-2">
							<span class="font-bold"> {containerId}: </span>
							<span>{count.toLocaleString()}</span>
						</div>
					{/if}
				{/each}
			</div>
		{:else}
			{item.static_id}
		{/if}
	{/snippet}
</List>
