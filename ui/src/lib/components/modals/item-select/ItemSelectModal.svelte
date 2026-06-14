<script lang="ts">
	import { Button, Card, Combobox, Input, Tooltip } from '$components/ui';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { itemsData, palsData } from '$lib/data';
	import { getAppState } from '$states';
	import { cn } from '$theme';
	import {
		ItemTypeA,
		ItemTypeB,
		PalGender,
		type DynamicItem,
		type EggConfig,
		type Item,
		type ItemGroup,
		type SelectOption
	} from '$types';
	import { staticIcons } from '$types/icons';
	import { assetLoader } from '$utils';
	import { focusModal } from '$utils/modalUtils';
	import { Apple, Cuboid, Delete, Gem, Save, Scroll, Shield, Sword, X } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import EggConfigSection from './EggConfigSection.svelte';
	import { getItemIcon, getItemTier, getBackgroundColor } from './itemUtils';

	let {
		title = '',
		itemId = '',
		count = 1,
		dynamicItem,
		group,
		closeModal
	}: {
		title: string;
		itemId: string;
		dynamicItem?: DynamicItem;
		count: number;
		group: ItemGroup;
		closeModal: (value?: [string, number, EggConfig]) => void;
	} = $props();

	let modalContainer: HTMLDivElement;

	const appState = getAppState();

	let eggConfig: EggConfig = $state({
		character_id: '',
		gender: PalGender.FEMALE,
		talent_hp: 0,
		talent_shot: 0,
		talent_defense: 0,
		learned_skills: [],
		active_skills: [],
		passive_skills: []
	});

	const selectedPalKey = $derived(eggConfig.character_id.replace('BOSS_', ''));
	const palOptions: SelectOption[] = $derived.by(() => {
		const item = itemsData.getByKey(itemId);
		if (!item || !item.details.dynamic?.character_ids) {
			if (!selectedPalKey) return [];
			const palData = palsData.getByKey(selectedPalKey);
			return [
				{
					label: palData?.localized_name || selectedPalKey,
					value: selectedPalKey
				}
			];
		}

		return item.details.dynamic.character_ids
			.map((charId) => {
				const palInfo = palsData.getByKey(charId.replace('BOSS_', ''));
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

	const itemData = $derived(itemsData.getByKey(itemId));
	const isEgg = $derived(itemData?.details.dynamic?.type === 'egg');

	const eggIconSrc = $derived.by(() => {
		if (!itemData || (itemData && itemData.details.dynamic?.type !== 'egg')) return;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`);
	});

	const palIconSrc = $derived.by(() => {
		if (!selectedPalKey) return staticIcons.unknownIcon;
		const palData = palsData.getByKey(selectedPalKey);
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
					return (
						item.details.group != 'KeyItem' ||
						(item.details.dynamic?.type === 'egg' &&
							item.details.dynamic?.character_ids &&
							item.details.dynamic?.character_ids?.length > 0)
					);
				default:
					return true;
			}
		});
	});
	const selectOptions: SelectOption[] = $derived.by(() => {
		return items.map((item) => ({ label: item.info.localized_name, value: item.id }));
	});

	const selectedItemMaxStackCount = $derived.by(() => {
		const item = itemsData.getByKey(itemId);
		if (!item) return 1;

		const maxStackCount = item.details.max_stack_count;
		if (!maxStackCount) return 1;

		return maxStackCount === 9999 && appState.settings.cheat_mode ? 999999999 : maxStackCount;
	});

	const cardClass = $derived(
		isEgg ? 'w-full max-w-[1200px] min-w-[600px]' : 'w-full w-[600px]'
	);
	const controlsClass = $derived(isEgg ? 'grid grid-cols-[570px_1fr] gap-2' : 'flex w-full');

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? [itemId, count, eggConfig] : undefined);
	}

	function handleClear() {
		itemId = 'None';
		count = 0;
	}

	onMount(() => {
		if (dynamicItem) {
			eggConfig.character_id = dynamicItem.character_id || '';
			eggConfig.active_skills = dynamicItem.active_skills || [];
			eggConfig.learned_skills = dynamicItem.learned_skills || [];
			eggConfig.passive_skills = dynamicItem.passive_skills || [];
			eggConfig.talent_defense = dynamicItem.talent_defense;
			eggConfig.talent_hp = dynamicItem.talent_hp;
			eggConfig.talent_shot = dynamicItem.talent_shot;
			eggConfig.gender = dynamicItem.gender as PalGender;
		}
		focusModal(modalContainer);
	});
</script>

{#snippet noIcon(typeA: ItemTypeA, typeB: ItemTypeB)}
	{#if typeA === ItemTypeA.Weapon}
		<Sword class="h-12 w-12"></Sword>
	{:else if typeA === ItemTypeA.Armor && typeB === ItemTypeB.Shield}
		<Shield class="h-12 w-12"></Shield>
	{:else if typeA === ItemTypeA.Blueprint}
		<Scroll class="h-12 w-12"></Scroll>
	{:else if typeA === ItemTypeA.Accessory}
		<Gem class="h-12 w-12"></Gem>
	{:else if typeA === ItemTypeA.Material}
		<Cuboid class="h-12 w-12"></Cuboid>
	{:else if typeA === ItemTypeA.Food}
		<Apple class="h-12 w-12"></Apple>
	{:else}
		<Cuboid class="h-12 w-12"></Cuboid>
	{/if}
{/snippet}

<div bind:this={modalContainer}>
	<Card class={cardClass}>
		<h3 class="h3">{title}</h3>
		<div class={controlsClass}>
			<div class="w-full">
				<div class="flex flex-row items-center">
					<Combobox options={selectOptions} bind:value={itemId} viewportClass="h-50 2xl:h-100">
						{#snippet selectOption(option)}
							{@const item = itemsData.getByKey(option.value as string)}
							{#await getItemIcon(option.value as string) then icon}
								<div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
									{#if icon}
										<div
											class={cn(
												'flex items-center justify-center',
												getBackgroundColor(option.value as string, items)
											)}
										>
											<img src={icon} alt={option.label} class="h-12 w-12" />
										</div>
									{:else}
										<div
											class={cn(
												'flex items-center justify-center',
												getBackgroundColor(option.value as string, items)
											)}
										>
											{@render noIcon(item!.details.type_a, item!.details.type_b)}
										</div>
									{/if}
									<div class="flex flex-col">
										<span class="font-bold">{option.label}</span>

										<span class="text-xs text-muted">{item?.info.description}</span>
									</div>
									<span class="text-xs self-start">{getItemTier(option.value as string, items)}</span>
								</div>
							{:catch}
								<div class="grid grid-cols-[auto_1fr_auto]">
									<div
										class={cn(
											'mr-2 flex items-center justify-center',
											getBackgroundColor(option.value as string, items)
										)}
									>
										{@render noIcon(item!.details.type_a, item!.details.type_b)}
									</div>
									<span class="h-6">{option.label}</span>
									<span>{getItemTier(option.value as string, items)}</span>
								</div>
							{/await}
						{/snippet}
					</Combobox>
					{#if !isEgg && selectedItemMaxStackCount && selectedItemMaxStackCount > 1}
						<Input
							labelClass="w-1/4 ml-1"
							type="number"
							bind:value={count}
							max={selectedItemMaxStackCount}
						/>
					{/if}
				</div>

				{#if isEgg}
					<div class="flex flex-col space-y-4">
						<div class="flex items-end space-x-2"></div>

						<div class="mt-4 flex items-center justify-center space-x-4 p-4">
							<div class="flex flex-col items-center">
								<span class="text-surface-400 mb-1 text-sm">{m.egg()}</span>
								<img
									src={eggIconSrc}
									alt={itemId || 'Unknown Egg'}
									class="h-20 w-20 object-contain 2xl:h-24 2xl:w-24"
								/>
							</div>
							<span class="text-surface-400 text-2xl font-bold">=</span>
							<div class="flex flex-col items-center">
								<span class="text-surface-400 mb-1 text-sm">{c.pal}</span>
								<div class="relative flex">
									<img
										src={palIconSrc}
										alt={eggConfig.character_id || 'Unknown Pal'}
										class="h-20 w-20 rounded-full object-contain 2xl:h-24 2xl:w-24"
									/>
									<div
										class={cn(
											'absolute -top-1 -right-4 h-6 w-6 xl:h-8 xl:w-8',
											eggConfig.gender == PalGender.MALE ? 'text-primary-300' : 'text-tertiary-300'
										)}
									>
										<img
											src={assetLoader.loadImage(
												`${ASSET_DATA_PATH}/img/${eggConfig.gender}.webp`
											)}
											alt={eggConfig.gender}
										/>
									</div>
								</div>
							</div>
						</div>
					</div>
				{/if}
			</div>
			{#if isEgg}
				<EggConfigSection bind:eggConfig {palOptions} />
			{/if}
		</div>

		<div class="mt-2 flex flex-row items-center space-x-2">
			<Tooltip position="bottom">
				<Button variant="ghost" size="icon" onclick={handleClear}>
					<Delete />
				</Button>
				{#snippet popup()}
					<span>{m.clear()}</span>
				{/snippet}
			</Tooltip>
			<Tooltip position="bottom">
				<Button variant="ghost" size="icon" onclick={() => handleClose(true)} data-modal-primary>
					<Save />
				</Button>
				{#snippet popup()}
					<span>{c.save}</span>
				{/snippet}
			</Tooltip>
			<Tooltip position="bottom">
				<Button variant="ghost" size="icon" onclick={() => handleClose(false)}>
					<X />
				</Button>
				{#snippet popup()}
					<span>{m.cancel()}</span>
				{/snippet}
			</Tooltip>
		</div>
	</Card>
</div>