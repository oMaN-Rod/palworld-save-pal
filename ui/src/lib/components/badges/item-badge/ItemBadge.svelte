<script lang="ts">
	import { Tooltip } from '$components/ui';
	import type { ContainerSlot, DynamicItemDetails, Item, ItemGroup, Tier } from '$types';
	import { assetLoader } from '$utils/asset-loader';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { itemsData, palsData } from '$lib/data';
	import { cn } from '$theme';
	import { getModalState } from '$states';
	import { ItemSelectModal } from '$components';
	import { Progress } from '@skeletonlabs/skeleton-svelte';
	import { Package } from 'lucide-svelte';

	let { slot = $bindable<ContainerSlot>(), itemGroup } = $props<{
		slot: ContainerSlot;
		itemGroup: ItemGroup;
	}>();

	const modal = getModalState();
	let item: Item | undefined = $state(undefined);
	let itemClass: string = $state('');
	let itemPopupHeaderClass: string = $state('');
	let itemPopupTierClass: string = $state('');
	let icon: string | undefined = $state('');
	let dynamic: DynamicItemDetails | undefined = $state(undefined);
	let palIcon: string | undefined = $state('');

	function getItemClass(tier: Tier | undefined) {
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

	function getItemPopupHeaderClass(tier: Tier | undefined) {
		switch (tier) {
			case 'Uncommon':
				return 'bg-gradient-to-tl from-green-500/50 to-green-800/50 text-green-300 border-green-500';
			case 'Rare':
				return 'bg-gradient-to-tl from-blue-500/50 to-blue-800/50 text-blue-300 border-blue-500';
			case 'Epic':
				return 'bg-gradient-to-tl from-purple-500/50 to-purple-800/50 text-purple-300 border-purple-500';
			case 'Legendary':
				return 'bg-gradient-to-tl from-yellow-500/50 to-yellow-800/50 text-yellow-300 border-yellow-500';
			default:
				return 'bg-surface-800';
		}
	}

	function getItemPopupTierClass(tier: Tier | undefined) {
		switch (tier) {
			case 'Uncommon':
				return 'bg-green-800 text-green-300 border-green-500';
			case 'Rare':
				return 'bg-blue-800 text-blue-300 border-blue-500';
			case 'Epic':
				return 'bg-purple-800 text-purple-300 border-purple-500';
			case 'Legendary':
				return 'bg-yellow-800 text-yellow-300 border-yellow-500';
			default:
				return 'bg-surface-900 text-gray-300 border-gray-500';
		}
	}

	async function getItemIcon(staticId: string) {
		if (!staticId || staticId == 'None') return;
		const itemData = await itemsData.searchItems(staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		const iconPath = `${ASSET_DATA_PATH}/img/icons/${itemData.details.image}.png`;
		const icon = await assetLoader.loadImage(iconPath);
		return icon;
	}

	async function getItemData(staticId: string) {
		if (!staticId || staticId == 'None') return;
		const itemData = await itemsData.searchItems(staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		return itemData;
	}

	async function getPalIcon(staticId: string) {
		if (!staticId || staticId == 'None' || !staticId.includes('SkillUnlock_')) return;
		const palName = staticId.replace('SkillUnlock_', '');
		const palData = await palsData.getPalInfo(palName);
		if (!palData) {
			console.error(`Pal data not found for static id: ${staticId}`);
			return;
		}
		const palImgName = palData.localized_name.toLowerCase().replaceAll(' ', '_');
		const iconPath = `${ASSET_DATA_PATH}/img/pals/menu/${palImgName}_menu.png`;
		const icon = await assetLoader.loadImage(iconPath);
		return icon;
	}

	async function getItemDynamicDetails(staticId: string) {
		if (!staticId || staticId == 'None') return;
		const itemData = await itemsData.searchItems(staticId);
		if (!itemData?.details.dynamic) {
			return;
		}
		return itemData.details.dynamic;
	}

	const initItem = async (staticId: string) => {
		if (!staticId) return;
		if (staticId == 'None') {
			item = undefined;
			icon = undefined;
			dynamic = undefined;
			return;
		}
		const data = await getItemData(staticId);
		icon = await getItemIcon(staticId);
		dynamic = await getItemDynamicDetails(staticId);
		palIcon = await getPalIcon(staticId);
		if (!data) return;
		item = data;
		itemClass = getItemClass(item.details.tier);
		itemPopupHeaderClass = getItemPopupHeaderClass(item.details.tier);
		itemPopupTierClass = getItemPopupTierClass(item.details.tier);
	};

	async function handleItemSelect() {
		// @ts-ignore
		const result = (await modal.showModal(ItemSelectModal, {
			group: itemGroup,
			itemId: slot.static_id,
			count: !slot.count || slot.count == 0 ? 1 : slot.count,
			title: 'Select Item'
		})) as [string, number] | undefined;
		if (result) {
			const [static_id, count] = result;
			console.log('Selected item:', static_id, count);
			slot.static_id = !static_id ? 'None' : static_id;
			slot.count = count;
			if (slot.static_id == 'None') {
				slot.dynamic_item = undefined;
				item = undefined;
				icon = undefined;
				dynamic = undefined;
				return;
			}
			const itemData = await getItemData(static_id);
			if (itemData?.details.dynamic) {
				if (!slot.dynamic_item) {
					slot.dynamic_item = {
						local_id: '00000000-0000-0000-0000-000000000000',
						durability: itemData.details.dynamic.durability,
						remaining_bullets: itemData.details.dynamic.magazine_size,
						type: itemData.details.dynamic.type
					};
				} else {
					slot.dynamic_item.durability = itemData.details.dynamic.durability;
					slot.dynamic_item.remaining_bullets = itemData.details.dynamic.magazine_size;
					slot.dynamic_item.type = itemData.details.dynamic.type;
				}
			}
			initItem(static_id);
		}
	}

	$effect(() => {
		console.log('Slot:', slot);
		initItem(slot.static_id);
	});
</script>

<button class="hover:ring-secondary-500 hover:ring" onclick={handleItemSelect}>
	{#if item}
		<Tooltip
			popupClass="p-0 mt-12 bg-surface-600"
			rounded="rounded-none"
			position="right"
			useArrow={false}
		>
			<div class="flex flex-col">
				<div
					class={cn(
						'bg-surface-800/50 relative flex h-16 w-16 items-center justify-center',
						itemClass
					)}
				>
					{#if icon}
						<enhanced:img
							src={icon}
							alt={item.info.localized_name}
							style="width: 62px; height: 62px;"
						></enhanced:img>
					{/if}
					{#if palIcon}
						<div class="bg-surface-800 border-surface-600 absolute right-0 top-0 h-7 w-7 border">
							<enhanced:img src={palIcon} alt="Pal Icon" class="h-full w-full object-cover"
							></enhanced:img>
						</div>
					{/if}
					{#if slot.count}
						<span class="absolute bottom-0 right-0.5 text-xs">{slot.count}</span>
					{/if}
				</div>
				{#if (dynamic && dynamic.type === 'weapon' && slot.dynamic_item) || (dynamic && dynamic.type === 'armor' && slot.dynamic_item)}
					<Progress value={slot.dynamic_item.durability} max={dynamic.durability} height="h-1" />
				{/if}
			</div>

			{#snippet popup()}
				<div class="flex w-96 flex-col">
					<div class={cn('flex flex-col space-y-2 border-b p-2', itemPopupHeaderClass)}>
						<h4 class="h4 text-left">{item?.info.localized_name}</h4>
						<div class="grid grid-cols-[1fr_auto] gap-2">
							<span class="grow text-left text-gray-300">
								{item?.details.type}
							</span>
							<div
								class={cn(
									'border-l border-r p-2 px-2 py-0.5 text-left text-sm font-bold',
									itemPopupTierClass
								)}
							>
								{item?.details.tier}
							</div>
						</div>
					</div>
					<div class="relative flex flex-row">
						{#if icon}
							<div class="m-4 ml-8">
								<enhanced:img
									src={icon}
									alt={item?.info.localized_name}
									style="width: 112px; height: 112px;"
								></enhanced:img>
							</div>
							<div
								class="bg-surface-800 text-one-surface hover:ring-secondary-500 absolute bottom-4 right-4 rounded px-3 py-1 font-semibold hover:ring"
								style="min-width: 80px; height: 2rem;"
							>
								<div class="relative z-10 flex h-full items-center justify-between">
									<span class="mr-8 text-xs">in inventory</span>
									<span class="font-bold">{slot.count}</span>
								</div>
								<span class="border-surface-700 absolute inset-0 rounded border"></span>
								<span class="bg-surface-400 absolute left-0 top-0 h-0.5 w-0.5"></span>
								<span class="bg-surface-400 absolute right-0 top-0 h-0.5 w-0.5"></span>
								<span class="bg-surface-400 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
								<span class="bg-surface-400 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
							</div>
						{/if}
					</div>
					<div class="bg-surface-900 p-2 text-left">
						<span class="whitespace-pre-line">{item?.info.description}</span>
					</div>
				</div>
			{/snippet}
		</Tooltip>
	{:else if slot.static_id !== 'None'}
		<Tooltip>
			<div class="bg-surface-800 relative flex h-16 w-16 items-center justify-center">
				<Package size="48" />
				<span class="absolute bottom-0 right-0 text-xs">{slot.count}</span>
			</div>

			{#snippet popup()}
				<span>{slot.static_id}</span>
			{/snippet}
		</Tooltip>
	{:else}
		<Tooltip>
			<div class="bg-surface-800 relative flex h-16 w-16 items-center justify-center"></div>

			{#snippet popup()}
				<span>Empty 😒</span>
			{/snippet}
		</Tooltip>
	{/if}
</button>
