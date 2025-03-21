<script lang="ts">
	import { Card, Tooltip, Combobox, Input } from '$components/ui';
	import { ItemTypeA, ItemTypeB, Rarity, type SelectOption } from '$types';
	import { Apple, Cuboid, Delete, Gem, Save, Scroll, Shield, Sword, X } from 'lucide-svelte';
	import { itemsData } from '$lib/data';
	import type { Item, ItemGroup } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { cn } from '$theme';
	import { assetLoader } from '$utils';

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

	let items: Item[] = $derived.by(() => {
		return Object.values(itemsData.items).filter((item) => {
			if (
				item.details.type_a == ItemTypeA.None ||
				item.details.type_a == ItemTypeA.MonsterEquipWeapon ||
				item.details.type_b == ItemTypeB.MaterialPalEgg ||
				item.details.disabled
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
				case 'SphereModule':
					return item.details.group == 'SphereModule';
				case 'Common':
					return item.details.group != 'KeyItem';
				default:
					return true;
			}
		});
	});
	let selectOptions: SelectOption[] = $derived.by(() => {
		return items.map((item) => ({ label: item.info.localized_name, value: item.id }));
	});

	let selectedItemMaxStackCount = $derived(
		items.find((item) => item.id === itemId)?.details.max_stack_count
	);

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? [itemId, count] : undefined);
	}

	function handleClear() {
		itemId = 'None';
		count = 0;
	}

	function getItemIcon(staticId: string) {
		if (!staticId) return;
		const itemData = itemsData.items[staticId];
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		if (!itemData.details.icon) {
			console.error(`Item icon not found for static id: ${staticId}`);
			return;
		}
		try {
			if (staticId.includes('SkillCard')) {
				return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${itemData.details.icon}.png`);
			} else {
				return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${itemData.details.icon}.png`);
			}
		} catch (error) {
			console.error(`Failed to load image for static id: ${staticId}`);
			return;
		}
	}

	function getItemTier(staticId: string) {
		if (!staticId) return;
		const itemData = items.find((item) => item.id === staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		return Rarity[itemData.details.rarity];
	}

	function getItem(staticId: string) {
		if (!staticId) return;
		const itemData = items.find((item) => item.id === staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		return itemData;
	}

	function getBackgroundColor(staticId: string) {
		if (!staticId) return;
		const itemData = items.find((item) => item.id === staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		const tier = itemData.details.rarity;
		switch (tier) {
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

{#snippet noIcon(typeA: ItemTypeA, typeB: ItemTypeB)}
	{#if typeA === ItemTypeA.Weapon}
		<Sword class="h-8 w-8"></Sword>
	{:else if typeA === ItemTypeA.Armor && typeB === ItemTypeB.Shield}
		<Shield class="h-8 w-8"></Shield>
	{:else if typeA === ItemTypeA.Blueprint}
		<Scroll class="h-8 w-8"></Scroll>
	{:else if typeA === ItemTypeA.Accessory}
		<Gem class="h-8 w-8"></Gem>
	{:else if typeA === ItemTypeA.Material}
		<Cuboid class="h-8 w-8"></Cuboid>
	{:else if typeA === ItemTypeA.Food}
		<Apple class="h-8 w-8"></Apple>
	{:else}
		<Cuboid class="h-8 w-8"></Cuboid>
	{/if}
{/snippet}

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<div class="flex flex-row items-center">
		<Combobox options={selectOptions} bind:value={itemId}>
			{#snippet selectOption(option)}
				{#await getItemIcon(option.value) then icon}
					{@const item = getItem(option.value)}
					<div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
						{#if icon}
							<div
								class={cn(
									'mr-2 flex items-center justify-center',
									getBackgroundColor(option.value)
								)}
							>
								<img src={icon} alt={option.label} class="h-8 w-8" />
							</div>
						{:else}
							<div
								class={cn(
									'mr-2 flex items-center justify-center',
									getBackgroundColor(option.value)
								)}
							>
								{@render noIcon(item!.details.type_a, item!.details.type_b)}
							</div>
						{/if}
						<div class="flex flex-col">
							<div class="flex space-x-4">
								<span class="grow items-center">{option.label}</span>
								<span class="text-xs">{getItemTier(option.value)}</span>
							</div>

							<span class="text-xs">{item?.info.description}</span>
						</div>
					</div>
				{:catch}
					{@const item = getItem(option.value)}
					<div class="grid grid-cols-[auto_1fr_auto]">
						<div
							class={cn('mr-2 flex items-center justify-center', getBackgroundColor(option.value))}
						>
							{@render noIcon(item!.details.type_a, item!.details.type_b)}
						</div>
						<span class="h-6">{option.label}</span>
						<span>{getItemTier(option.value)}</span>
					</div>
				{/await}
			{/snippet}
		</Combobox>
		<Input labelClass="w-1/4" type="number" bind:value={count} max={selectedItemMaxStackCount} />
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
