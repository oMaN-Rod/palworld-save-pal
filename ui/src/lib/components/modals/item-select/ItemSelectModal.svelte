<script lang="ts">
	import { Card, Tooltip, Combobox, Input } from '$components/ui';
	import type { ItemType, SelectOption, Tier } from '$types';
	import { Apple, Cuboid, Delete, Gem, Pizza, Save, Scroll, Shield, Sword, X } from 'lucide-svelte';
	import { itemsData } from '$lib/data';
	import type { Item, ItemGroup } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';

	let {
		title = '',
		itemId = '',
		count = 1,
		group,
		closeModal
	} = $props<{
		title: string;
		itemId: string;
		count: number;
		group: ItemGroup;
		closeModal: (value: [string, number]) => void;
	}>();

	let selectOptions: SelectOption[] = $state([]);
	let items: Item[] = $state([]);

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? [itemId, count] : undefined);
	}

	function handleClear() {
		itemId = 'None';
		count = 0;
	}

	async function getItems() {
		const allItems = await itemsData.getAllItems();
		const applicableItems = allItems.filter((item) => {
			if (
				item.details.type == 'Structure' ||
				item.details.type == 'Egg' ||
				item.details.type == 'Unknown' ||
				item.details.type == 'None'
			) {
				return false;
			}
			switch (group as ItemGroup) {
				case 'Accessory':
					return item.details.group == 'Accessory';
				case 'Body':
					return item.details.group == 'Body';
				case 'Food':
					return item.details.group == 'Food';
				case 'Glider':
					return item.details.group == 'Glider';
				case 'Head':
					return item.details.group == 'Head';
				case 'Shield':
					return item.details.group == 'Shield';
				case 'Weapon':
					return item.details.group == 'Weapon';
				case 'KeyItem':
					return item.details.group == 'KeyItem';
				case 'Common':
					return item.details.group != 'KeyItem';
				default:
					return true;
			}
		});
		items = applicableItems;
		selectOptions = items.map((item) => ({ label: item.info.localized_name, value: item.id }));
	}

	$effect(() => {
		getItems();
	});

	async function getItemIcon(staticId: string) {
		if (!staticId) return;
		const itemData = await itemsData.searchItems(staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		const iconPath = `${ASSET_DATA_PATH}/img/icons/${itemData.details.image}.png`;
		const icon = await assetLoader.loadImage(iconPath);
		return icon;
	}

	function getItemTier(staticId: string) {
		if (!staticId) return;
		const itemData = items.find((item) => item.id === staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		return itemData.details.tier;
	}

	function getItemType(staticId: string) {
		if (!staticId) return;
		const itemData = items.find((item) => item.id === staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		return itemData.details.type;
	}

	function getBackgroundColor(staticId: string) {
		if (!staticId) return;
		const itemData = items.find((item) => item.id === staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		const tier = itemData.details.tier;
		switch (tier) {
			case 'Uncommon':
				return 'bg-gradient-to-tl from-green-500/50';
			case 'Rare':
				return 'bg-gradient-to-tl from-blue-500/50';
			case 'Epic':
				return 'bg-gradient-to-tl from-purple-500/50';
			case 'Legendary':
				return 'bg-gradient-to-tl from-yellow-500/50';
			default:
				return '';
		}
	}
</script>

{#snippet noIcon(itemType: ItemType | undefined)}
	{#if itemType === 'Weapon'}
		<Sword class="h-6 w-6"></Sword>
	{:else if itemType === 'Armor'}
		<Shield class="h-6 w-6"></Shield>
	{:else if itemType === 'Schematic'}
		<Scroll class="h-6 w-6"></Scroll>
	{:else if itemType === 'Accessory'}
		<Gem class="h-6 w-6"></Gem>
	{:else if itemType === 'Material'}
		<Cuboid class="h-6 w-6"></Cuboid>
	{:else if itemType === 'Ingredient'}
		<Apple class="h-6 w-6"></Apple>
	{:else}
		<Cuboid class="h-6 w-6"></Cuboid>
	{/if}
{/snippet}

<Card class="bg-surface-500 min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<div class="flex flex-row">
		<Combobox options={selectOptions} bind:value={itemId}>
			{#snippet selectOption(option)}
				{#await getItemIcon(option.value) then icon}
					{@const itemType = getItemType(option.value)}
					<div class="grid grid-cols-[auto_1fr_auto_auto]">
						{#if icon}
							<div
								class={cn(
									'mr-2 flex items-center justify-center',
									getBackgroundColor(option.value)
								)}
							>
								<enhanced:img src={icon} alt={option.label} class="h-6 w-6"></enhanced:img>
							</div>
						{:else}
							<div
								class={cn(
									'mr-2 flex items-center justify-center',
									getBackgroundColor(option.value)
								)}
							>
								{@render noIcon(itemType)}
							</div>
						{/if}
						<span class="h-6">{option.label}</span>
						<span>{getItemTier(option.value)}</span>
					</div>
				{:catch}
					<div class="grid grid-cols-[auto_1fr_auto]">
						<div
							class={cn('mr-2 flex items-center justify-center', getBackgroundColor(option.value))}
						>
							<!-- svelte-ignore element_invalid_self_closing_tag -->
							{@render noIcon(getItemType(option.value))}
						</div>
						<span class="h-6">{option.label}</span>
						<span>{getItemTier(option.value)}</span>
					</div>
				{/await}
			{/snippet}
		</Combobox>
		<Input labelClass="w-1/4" type="number" bind:value={count} />
	</div>

	<div class="mt-2 flex flex-row items-center space-x-2">
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={handleClear}>
				<Delete />
			</button>
			{#snippet popup()}
				<span>Clear</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(true)}>
				<Save />
			</button>
			{#snippet popup()}
				<span>Save</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(false)}>
				<X />
			</button>
			{#snippet popup()}
				<span>Cancel</span>
			{/snippet}
		</Tooltip>
	</div>
</Card>
