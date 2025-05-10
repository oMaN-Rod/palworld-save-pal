<script lang="ts">
	import { Card, Tooltip, Combobox, Input } from '$components/ui';
	import { ItemTypeA, ItemTypeB, Rarity, type SelectOption } from '$types';
	import {
		Apple,
		Cuboid,
		Delete,
		Gem,
		Save,
		Scroll,
		Shield,
		Sword,
		Trash2,
		X
	} from 'lucide-svelte';
	import { itemsData, palsData } from '$lib/data';
	import type { Item, ItemGroup } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { cn } from '$theme';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';
	import { onMount } from 'svelte';

	let {
		title = '',
		itemId = '',
		count = 1,
		palId,
		group,
		closeModal
	} = $props<{
		title: string;
		itemId: string;
		palId?: string;
		count: number;
		group: ItemGroup;
		closeModal: (value: [string, number]) => void;
	}>();

	let selectedPalId: string = $state('');
	const selectedPalKey = $derived(selectedPalId.replace('BOSS_', ''));
	const palOptions: SelectOption[] = $derived.by(() => {
		const item = itemsData.items[itemId];
		if (!item || !item.details.dynamic?.character_ids) return [];

		return item.details.dynamic.character_ids
			.map((charId) => {
				const palInfo = palsData.pals[charId.replace('BOSS_', '')];
				const label = charId.includes('BOSS_')
					? `${palInfo?.localized_name} (Alpha)`
					: palInfo?.localized_name || charId.replace('BOSS_', '');
				return {
					label: label,
					value: charId
				};
			})
			.sort((a, b) => a.label.localeCompare(b.label));
	});
	const itemData = $derived.by(() => itemsData.items[itemId]);
	const isEgg = $derived(itemData?.details.dynamic?.type === 'egg');

	const eggIconSrc = $derived.by(() => {
		if (!itemData || (itemData && itemData.details.dynamic?.type !== 'egg')) return;
		if (itemData && !itemData.details.dynamic?.character_ids) return staticIcons.unknownEggIcon;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${itemData.details.icon}.png`);
	});

	const palIconSrc = $derived.by(() => {
		if (!selectedPalKey) return staticIcons.unknownIcon;
		const palData = palsData.pals[selectedPalKey];
		return assetLoader.loadMenuImage(selectedPalKey, palData?.is_pal ?? true);
	});

	const items: Item[] = $derived.by(() => {
		return Object.values(itemsData.items).filter((item) => {
			if (
				item.details.type_a == ItemTypeA.None ||
				item.details.type_a == ItemTypeA.MonsterEquipWeapon ||
				item.details.disabled
			) {
				return false;
			}
			if (
				item.details.dynamic?.type === 'egg' &&
				item.details.dynamic?.character_ids &&
				item.details.dynamic?.character_ids?.length > 0
			) {
				return true;
			} else if (item.details.dynamic?.type === 'egg') {
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
	const selectOptions: SelectOption[] = $derived.by(() => {
		return items.map((item) => ({ label: item.info.localized_name, value: item.id }));
	});

	const selectedItemMaxStackCount = $derived(
		items.find((item) => item.id === itemId)?.details.max_stack_count
	);

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? [itemId, count, selectedPalId] : undefined);
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

	function getPalIcon(palId: string): string {
		if (!palId) return staticIcons.unknownIcon;
		const palData = palsData.pals[palId];
		return assetLoader.loadMenuImage(palId, palData?.is_pal ?? true);
	}

	onMount(() => {
		if (palId) {
			selectedPalId = palId;
		}
	});
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
		{#if !isEgg && selectedItemMaxStackCount && selectedItemMaxStackCount > 1}
			<Input labelClass="w-1/4" type="number" bind:value={count} max={selectedItemMaxStackCount} />
		{/if}
	</div>

	{#if isEgg}
		<div class="flex flex-col space-y-4">
			<div class="flex items-end space-x-2">
				<Combobox
					label="Pal"
					options={palOptions}
					bind:value={selectedPalId}
					placeholder="Choose a Pal..."
				>
					{#snippet selectOption(option)}
						<div class="flex items-center space-x-2">
							<img src={getPalIcon(option.value)} alt={option.label} class="h-8 w-8" />
							<span>{option.label}</span>
						</div>
					{/snippet}
				</Combobox>
				<Tooltip label="Clear Pal Selection">
					<button
						class="btn hover:bg-error-500/25 p-2 disabled:cursor-not-allowed disabled:opacity-50"
						onclick={() => {
							selectedPalId = '';
						}}
						disabled={!selectedPalId}
					>
						<Trash2 size={20} />
					</button>
				</Tooltip>
			</div>

			<div class="mt-4 flex items-center justify-center space-x-4 p-4">
				<div class="flex flex-col items-center">
					<span class="text-surface-400 mb-1 text-sm">Egg</span>
					<img
						src={eggIconSrc}
						alt={itemId || 'Unknown Egg'}
						class="h-20 w-20 object-contain 2xl:h-24 2xl:w-24"
					/>
				</div>
				<span class="text-surface-400 text-2xl font-bold">=</span>
				<div class="flex flex-col items-center">
					<span class="text-surface-400 mb-1 text-sm">Pal</span>
					<img
						src={palIconSrc}
						alt={selectedPalId || 'Unknown Pal'}
						class="h-20 w-20 rounded-full object-contain 2xl:h-24 2xl:w-24"
					/>
				</div>
			</div>
		</div>
	{/if}

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
