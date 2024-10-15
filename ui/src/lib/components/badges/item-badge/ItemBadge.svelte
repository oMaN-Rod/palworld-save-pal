<script lang="ts">
	import { Tooltip } from '$components/ui';
	import {
		type ItemContainerSlot,
		type DynamicItemDetails,
		type Item,
		type ItemGroup,
		Rarity
	} from '$types';
	import { assetLoader } from '$utils/asset-loader';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { itemsData, palsData } from '$lib/data';
	import { cn } from '$theme';
	import { getModalState } from '$states';
	import { ItemSelectModal } from '$components';
	import { Progress } from '@skeletonlabs/skeleton-svelte';
	import { Package } from 'lucide-svelte';

	let {
		slot = $bindable<ItemContainerSlot>(),
		itemGroup,
		onCopyPaste
	} = $props<{
		slot: ItemContainerSlot;
		itemGroup: ItemGroup;
		onCopyPaste?: (event: MouseEvent) => void;
	}>();

	const modal = getModalState();
	let item: Item | undefined = $state(undefined);
	let itemClass: string = $state('');
	let itemPopupHeaderClass: string = $state('');
	let itemPopupTierClass: string = $state('');
	let icon: string | undefined = $state('');
	let rightClickIcon: string | undefined = $state('');
	let middleClickIcon: string | undefined = $state('');
	let ctrlIcon: string | undefined = $state('');
	let weightIcon: string | undefined = $state('');
	let sadIcon: string | undefined = $state('');
	let dynamic: DynamicItemDetails | undefined = $state(undefined);
	let palIcon: string | undefined = $state('');

	let slotWeight = $derived((item?.details.weight * slot.count).toFixed(1));

	function getItemClass(tier: Rarity | undefined) {
		switch (tier) {
			case Rarity.Uncommon:
				return 'bg-gradient-to-tl from-green-500/50';
			case Rarity.Rare:
				return 'bg-gradient-to-tl from-blue-500/50';
			case Rarity.Epic:
				return 'bg-gradient-to-tl from-purple-500/50';
			case Rarity.Legendary:
				return 'bg-gradient-to-tl from-yellow-500/50';
			default:
				return '';
		}
	}

	function getItemPopupHeaderClass(tier: Rarity | undefined) {
		switch (tier) {
			case Rarity.Uncommon:
				return 'bg-gradient-to-tl from-green-500/50 to-green-800/50 text-green-300 border-green-500';
			case Rarity.Rare:
				return 'bg-gradient-to-tl from-blue-500/50 to-blue-800/50 text-blue-300 border-blue-500';
			case Rarity.Epic:
				return 'bg-gradient-to-tl from-purple-500/50 to-purple-800/50 text-purple-300 border-purple-500';
			case Rarity.Legendary:
				return 'bg-gradient-to-tl from-yellow-500/50 to-yellow-800/50 text-yellow-300 border-yellow-500';
			default:
				return 'bg-surface-800';
		}
	}

	function getItemPopupTierClass(tier: Rarity | undefined) {
		switch (tier) {
			case Rarity.Uncommon:
				return 'bg-green-800 text-green-300 border-green-500';
			case Rarity.Rare:
				return 'bg-blue-800 text-blue-300 border-blue-500';
			case Rarity.Epic:
				return 'bg-purple-800 text-purple-300 border-purple-500';
			case Rarity.Legendary:
				return 'bg-yellow-800 text-yellow-300 border-yellow-500';
			default:
				return 'bg-surface-900 text-gray-300 border-gray-500';
		}
	}

	async function getStaticIcons() {
		const rightClickIconPath = `${ASSET_DATA_PATH}/img/actions/MouseButtonRight.png`;
		const middleClickIconPath = `${ASSET_DATA_PATH}/img/actions/MouseWheelButton.png`;
		const ctrlIconPath = `${ASSET_DATA_PATH}/img/actions/Keyboard_Ctrl.png`;
		const weightIconPath = `${ASSET_DATA_PATH}/img/icons/weight.png`;
		const sadIconPath = `${ASSET_DATA_PATH}/img/icons/Cattiva_Pleading.png`;
		rightClickIcon = await assetLoader.loadImage(rightClickIconPath);
		middleClickIcon = await assetLoader.loadImage(middleClickIconPath);
		ctrlIcon = await assetLoader.loadImage(ctrlIconPath);
		weightIcon = await assetLoader.loadImage(weightIconPath);
		sadIcon = await assetLoader.loadImage(sadIconPath);
	}

	async function getItemIcon(staticId: string) {
		if (!staticId || staticId == 'None') return;
		const itemData = await itemsData.searchItems(staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		let iconPath: string;
		if (staticId.includes('SkillCard')) {
			iconPath = `${ASSET_DATA_PATH}/img/elements/${itemData.details.icon}.png`;
		} else {
			iconPath = `${ASSET_DATA_PATH}/img/icons/${itemData.details.icon}.png`;
		}
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
		itemClass = getItemClass(item.details.rarity);
		itemPopupHeaderClass = getItemPopupHeaderClass(item.details.rarity);
		itemPopupTierClass = getItemPopupTierClass(item.details.rarity);
	};

	async function handleItemSelect() {
		// @ts-ignore
		const result = await modal.showModal<[string, number]>(ItemSelectModal, {
			group: itemGroup,
			itemId: slot.static_id,
			count: !slot.count || slot.count == 0 ? 1 : slot.count,
			title: 'Select Item'
		});
		if (!result) return;
		const [static_id, count] = result;
		slot.static_id = !static_id ? 'None' : static_id;
		if (slot.static_id == 'None') {
			slot.count = 0;
			slot.dynamic_item = undefined;
			item = undefined;
			icon = undefined;
			dynamic = undefined;
			return;
		}
		const itemData = await getItemData(static_id);
		if (itemData) {
			slot.count =
				count > itemData.details.max_stack_count ? itemData.details.max_stack_count : count;
			if (itemData.details.dynamic) {
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
		}

		initItem(static_id);
	}

	$effect(() => {
		initItem(slot.static_id);
	});

	$effect(() => {
		getStaticIcons();
	});
</script>

<button
	class="hover:ring-secondary-500 w-12 hover:ring xl:w-16"
	onclick={handleItemSelect}
	oncontextmenu={(event) => event.preventDefault()}
	onmousedown={(event) => onCopyPaste(event)}
>
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
						'bg-surface-800/50 relative flex h-12 w-12 items-center justify-center xl:h-16 xl:w-16',
						itemClass
					)}
				>
					<span class="absolute left-0.5 top-0 text-xs">{slotWeight}</span>
					{#if icon}
						<enhanced:img
							src={icon}
							alt={item.info.localized_name}
							class="h-12 w-12 xl:h-16 xl:w-16"
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
					<Progress
						value={slot.dynamic_item.durability}
						max={dynamic.durability < slot.dynamic_item.durability
							? slot.dynamic_item.durability
							: dynamic.durability}
						height="h-1"
					/>
				{/if}
			</div>

			{#snippet popup()}
				<div class="flex w-96 flex-col">
					<div class={cn('flex flex-col space-y-2 border-b p-2', itemPopupHeaderClass)}>
						<h4 class="h4 text-left">{item?.info.localized_name}</h4>
						<div class="grid grid-cols-[1fr_auto] gap-2">
							<span class="grow text-left text-gray-300">
								{item?.details.type_a}
							</span>
							<div
								class={cn(
									'border-l border-r p-2 px-2 py-0.5 text-left text-sm font-bold',
									itemPopupTierClass
								)}
							>
								{item?.details.rarity !== undefined ? Rarity[item.details.rarity] : ''}
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
					<div class="bg-surface-900 flex p-2 text-sm">
						<div class="flex grow flex-col space-y-2">
							<div class="flex items-center space-x-2">
								<div class="h-6 w-6">
									{#if rightClickIcon}
										<enhanced:img src={rightClickIcon} alt="Right Click" class="h-full w-full"
										></enhanced:img>
									{/if}
								</div>
								<span class="text-xs font-bold">Copy</span>
							</div>
							<div class="flex items-center space-x-2">
								<div class="h-6 w-6">
									{#if ctrlIcon}
										<enhanced:img src={ctrlIcon} alt="Right Click" class="h-full w-full"
										></enhanced:img>
									{/if}
								</div>
								<div class="h-6 w-6">
									{#if rightClickIcon}
										<enhanced:img src={rightClickIcon} alt="Right Click" class="h-full w-full"
										></enhanced:img>
									{/if}
								</div>
								<span class="text-xs font-bold">Paste</span>
							</div>
							<div class="flex items-center space-x-2">
								<div class="h-6 w-6">
									{#if ctrlIcon}
										<enhanced:img src={ctrlIcon} alt="Right Click" class="h-full w-full"
										></enhanced:img>
									{/if}
								</div>
								<div class="h-6 w-6">
									{#if middleClickIcon}
										<enhanced:img src={middleClickIcon} alt="Right Click" class="h-full w-full"
										></enhanced:img>
									{/if}
								</div>
								<span class="text-xs font-bold">Delete</span>
							</div>
						</div>
						<div
							class="bg-surface-800 text-one-surface hover:ring-secondary-500 absolute bottom-4 right-4 rounded px-3 py-1 font-semibold hover:ring"
							style="min-width: 80px; height: 2rem;"
						>
							<div class="relative z-10 flex h-full items-center justify-between">
								{#if weightIcon}
									<div class="h-6 w-6">
										<enhanced:img src={weightIcon} alt="Weight" class="h-full w-full"
										></enhanced:img>
									</div>
								{/if}
								<span class="font-bold">{slotWeight}</span>
							</div>
							<span class="border-surface-700 absolute inset-0 rounded border"></span>
							<span class="bg-surface-400 absolute left-0 top-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-400 absolute right-0 top-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-400 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-400 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
						</div>
					</div>
				</div>
			{/snippet}
		</Tooltip>
	{:else if slot.static_id !== 'None'}
		<Tooltip>
			<div
				class="bg-surface-800 relative flex h-12 w-12 items-center justify-center xl:h-16 xl:w-16"
			>
				<Package size="48" />
				<span class="absolute bottom-0 right-0 text-xs">{slot.count}</span>
			</div>

			{#snippet popup()}
				<span>{slot.static_id}</span>
			{/snippet}
		</Tooltip>
	{:else}
		<Tooltip>
			<div
				class="bg-surface-800 relative flex h-12 w-12 items-center justify-center xl:h-16 xl:w-16"
			></div>

			{#snippet popup()}
				<div class="flex">
					<span>Empty </span>
					{#if sadIcon}
						<enhanced:img src={sadIcon} alt="Sad Icon" class="h-6 w-6"></enhanced:img>
					{:else}
						<span>😢</span>
					{/if}
				</div>
			{/snippet}
		</Tooltip>
	{/if}
</button>

<style lang="postcss">
	.keyboard-shortcut {
		@apply kbd flex h-10 w-10 items-center justify-center p-1;
	}
</style>
