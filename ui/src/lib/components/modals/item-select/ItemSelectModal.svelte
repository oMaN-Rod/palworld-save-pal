<script lang="ts">
	import { Card, Tooltip, Combobox, Input, List, CornerDotButton } from '$components/ui';
	import {
		ItemTypeA,
		ItemTypeB,
		Rarity,
		type DynamicItem,
		type EggConfig,
		type Pal,
		type SelectOption
	} from '$types';
	import {
		Apple,
		Cuboid,
		Delete,
		Gem,
		Save,
		Scroll,
		Shield,
		Sword,
		TimerReset,
		Trash,
		Trash2,
		X
	} from 'lucide-svelte';
	import { activeSkillsData, itemsData, palsData, passiveSkillsData } from '$lib/data';
	import { PalGender, type Item, type ItemGroup } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { cn } from '$theme';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';
	import { onMount } from 'svelte';
	import { ActiveSkillOption, PassiveSkillOption, Talents } from '$components';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';

	let {
		title = '',
		itemId = '',
		count = 1,
		dynamicItem,
		group,
		closeModal
	} = $props<{
		title: string;
		itemId: string;
		dynamicItem?: DynamicItem;
		count: number;
		group: ItemGroup;
		closeModal: (value: [string, number]) => void;
	}>();

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
	let accordionValue = $state<string[]>(['pal']);

	const selectedPalKey = $derived(eggConfig.character_id.replace('BOSS_', ''));
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
	const activeSkillOptions: SelectOption[] = $derived(
		Object.values(activeSkillsData.activeSkills)
			.filter((skill) => {
				if (skill.id.toLowerCase().includes(`unique_${eggConfig.character_id.toLowerCase()}`)) {
					return true;
				}
				if (!skill.id.toLowerCase().includes('unique_')) {
					return true;
				}
				return false;
			})
			.filter(
				(aSkill) => !Object.values(eggConfig.active_skills).some((skill) => skill === aSkill.id)
			)
			.sort((a, b) => a.details.element.localeCompare(b.details.element))
			.map((s) => ({
				value: s.id,
				label: s.localized_name
			}))
	);
	const learnedSkillsOptions: SelectOption[] = $derived(
		Object.values(activeSkillsData.activeSkills)
			.filter((skill) => {
				if (skill.id.toLowerCase().includes(`unique_${eggConfig.character_id.toLowerCase()}`)) {
					return true;
				}
				if (!skill.id.toLowerCase().includes('unique_')) {
					return true;
				}
				return false;
			})
			.filter(
				(aSkill) => !Object.values(eggConfig.learned_skills).some((skill) => skill === aSkill.id)
			)
			.sort((a, b) => a.details.element.localeCompare(b.details.element))
			.map((s) => ({
				value: s.id,
				label: s.localized_name
			}))
	);
	const passiveSkillOptions: SelectOption[] = $derived(
		Object.values(passiveSkillsData.passiveSkills)
			.filter((pSkill) => !Object.values(eggConfig.passive_skills).some((p) => p === pSkill.id))
			.sort((a, b) => b.details.rank - a.details.rank)
			.map((s) => ({
				value: s.id,
				label: s.localized_name
			}))
	);
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

	const selectedItemMaxStackCount = $derived(
		items.find((item) => item.id === itemId)?.details.max_stack_count
	);

	const cardClass = $derived(isEgg ? 'w-[1200px]' : 'w-[600px]');
	const controlsClass = $derived(isEgg ? 'grid grid-cols-[570px_1fr] gap-2' : 'flex w-full');

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? [itemId, count, eggConfig] : undefined);
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
		if (dynamicItem) {
			eggConfig.character_id = dynamicItem.character_id;
			eggConfig.active_skills = dynamicItem.active_skills;
			eggConfig.learned_skills = dynamicItem.learned_skills;
			eggConfig.passive_skills = dynamicItem.passive_skills;
			eggConfig.talent_defense = dynamicItem.talent_defense;
			eggConfig.talent_hp = dynamicItem.talent_hp;
			eggConfig.talent_shot = dynamicItem.talent_shot;
			eggConfig.gender = dynamicItem.gender;
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

<Card class={cardClass}>
	<h3 class="h3">{title}</h3>
	<div class={controlsClass}>
		<div class="w-full">
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
									class={cn(
										'mr-2 flex items-center justify-center',
										getBackgroundColor(option.value)
									)}
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
					<Input
						labelClass="w-1/4"
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
							<div class="relative flex">
								<img
									src={palIconSrc}
									alt={eggConfig.character_id || 'Unknown Pal'}
									class="h-20 w-20 rounded-full object-contain 2xl:h-24 2xl:w-24"
								/>
								<div
									class={cn(
										'absolute -right-4 -top-1 h-6 w-6 xl:h-8 xl:w-8',
										eggConfig.gender == PalGender.MALE ? 'text-primary-300' : 'text-tertiary-300'
									)}
								>
									<img
										src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${eggConfig.gender}.png`)}
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
			<Accordion
				classes="w-full"
				value={accordionValue}
				onValueChange={(e) => (accordionValue = e.value)}
				collapsible
			>
				<Accordion.Item
					controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
					value="pal"
					controlHover="hover:bg-secondary-500/25"
				>
					{#snippet control()}
						Pal
					{/snippet}
					{#snippet panel()}
						<div class="flex w-full items-center space-x-2">
							<Combobox
								options={palOptions}
								bind:value={eggConfig.character_id}
								placeholder="Choose a Pal..."
							>
								{#snippet selectOption(option)}
									<div class="flex items-center space-x-2">
										<img src={getPalIcon(option.value)} alt={option.label} class="h-8 w-8" />
										<span>{option.label}</span>
									</div>
								{/snippet}
							</Combobox>
							<CornerDotButton
								onClick={() => {
									eggConfig.gender =
										eggConfig.gender === PalGender.FEMALE ? PalGender.MALE : PalGender.FEMALE;
								}}
								class="h-8 w-8 p-1"
							>
								<img
									src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${eggConfig.gender}.png`)}
									alt={eggConfig.gender}
								/>
							</CornerDotButton>
						</div>
					{/snippet}
				</Accordion.Item>
				<Accordion.Item
					controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
					value="active_skills"
					controlHover="hover:bg-secondary-500/25"
				>
					{#snippet control()}
						Active Skills
					{/snippet}
					{#snippet panel()}
						<Combobox
							options={activeSkillOptions}
							placeholder="Choose Active Skills..."
							onChange={(value) => {
								eggConfig.active_skills.push(value as string);
							}}
							disabled={eggConfig.active_skills.length >= 3}
						>
							{#snippet selectOption(option)}
								<ActiveSkillOption {option} />
							{/snippet}
						</Combobox>

						{#if eggConfig.active_skills.length > 0}
							<List
								items={eggConfig.active_skills}
								listClass="max-h-60 overflow-y-auto"
								canSelect={false}
								multiple={false}
							>
								{#snippet listHeader()}
									<div>
										<span class="font-bold">Active Skills</span>
									</div>
								{/snippet}
								{#snippet listItem(skill)}
									{@const activeSkill = activeSkillsData.activeSkills[skill]}
									<ActiveSkillOption option={{ label: activeSkill.localized_name, value: skill }} />
								{/snippet}
								{#snippet listItemActions(skill)}
									<button
										class="btn hover:bg-error-500/25 p-2"
										onclick={() =>
											(eggConfig.active_skills = eggConfig.active_skills.filter(
												(s) => s !== skill
											))}
									>
										<Trash size={16} />
									</button>
								{/snippet}
								{#snippet listItemPopup(skill)}
									{@const activeSkill = activeSkillsData.activeSkills[skill]}
									<div class="flex items-center space-x-1 justify-self-start">
										<TimerReset class="h-4 w-4" />
										<span class="font-bold">{activeSkill?.details.cool_time}</span>
										<span class="text-xs">Pwr</span>
										<span class="font-bold">{activeSkill?.details.power}</span>
									</div>
								{/snippet}
							</List>
						{/if}
					{/snippet}
				</Accordion.Item>
				<Accordion.Item
					controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
					value="learned_skills"
					controlHover="hover:bg-secondary-500/25"
				>
					{#snippet control()}
						Learned Skills
					{/snippet}
					{#snippet panel()}
						<Combobox
							options={learnedSkillsOptions}
							placeholder="Choose Learned Skills..."
							onChange={(value) => {
								eggConfig.learned_skills.push(value as string);
							}}
						>
							{#snippet selectOption(option)}
								<ActiveSkillOption {option} />
							{/snippet}
						</Combobox>

						{#if eggConfig.learned_skills.length > 0}
							<List
								items={eggConfig.learned_skills}
								listClass="max-h-60 overflow-y-auto"
								canSelect={false}
								multiple={false}
							>
								{#snippet listHeader()}
									<div>
										<span class="font-bold">Learned Skills</span>
									</div>
								{/snippet}
								{#snippet listItem(skill)}
									{@const activeSkill = activeSkillsData.activeSkills[skill]}
									<ActiveSkillOption option={{ label: activeSkill.localized_name, value: skill }} />
								{/snippet}
								{#snippet listItemActions(skill)}
									<button
										class="btn hover:bg-error-500/25 p-2"
										onclick={() =>
											(eggConfig.learned_skills = eggConfig.learned_skills.filter(
												(s) => s !== skill
											))}
									>
										<Trash size={16} />
									</button>
								{/snippet}
								{#snippet listItemPopup(skill)}
									{@const activeSkill = activeSkillsData.activeSkills[skill]}
									<div class="flex items-center space-x-1 justify-self-start">
										<TimerReset class="h-4 w-4" />
										<span class="font-bold">{activeSkill?.details.cool_time}</span>
										<span class="text-xs">Pwr</span>
										<span class="font-bold">{activeSkill?.details.power}</span>
									</div>
								{/snippet}
							</List>
						{/if}
					{/snippet}
				</Accordion.Item>
				<Accordion.Item
					controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
					value="passive_skills"
					controlHover="hover:bg-secondary-500/25"
				>
					{#snippet control()}
						Passive Skills
					{/snippet}
					{#snippet panel()}
						<Combobox
							label="Passive Skills"
							options={passiveSkillOptions}
							placeholder="Choose Passive Skills..."
							onChange={(value) => eggConfig.passive_skills.push(value as string)}
							disabled={eggConfig.passive_skills.length >= 4}
						>
							{#snippet selectOption(option)}
								<PassiveSkillOption {option} />
							{/snippet}
						</Combobox>

						{#if eggConfig.passive_skills.length > 0}
							<List
								items={eggConfig.passive_skills}
								listClass="max-h-60 overflow-y-auto"
								canSelect={false}
								multiple={false}
							>
								{#snippet listHeader()}
									<div>
										<span class="font-bold">Passive Skills</span>
									</div>
								{/snippet}
								{#snippet listItem(skill)}
									{@const passiveSkill = passiveSkillsData.passiveSkills[skill]}
									<PassiveSkillOption
										option={{ label: passiveSkill.localized_name, value: skill }}
									/>
								{/snippet}
								{#snippet listItemActions(skill)}
									<button
										class="btn hover:bg-error-500/25 p-2"
										onclick={() =>
											(eggConfig.passive_skills = eggConfig.passive_skills.filter(
												(s) => s !== skill
											))}
									>
										<Trash size={16} />
									</button>
								{/snippet}
								{#snippet listItemPopup(skill)}
									{@const passiveSkill = passiveSkillsData.passiveSkills[skill]}
									<div class="flex grow flex-col">
										<span class="grow truncate">{passiveSkill.localized_name}</span>
										<span class="text-xs">{passiveSkill?.description}</span>
									</div>
								{/snippet}
							</List>
						{/if}
					{/snippet}
				</Accordion.Item>
				<Accordion.Item
					controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
					value="talents"
					controlHover="hover:bg-secondary-500/25"
				>
					{#snippet control()}
						IVs
					{/snippet}
					{#snippet panel()}
						<Talents bind:pal={eggConfig as Pal} />
					{/snippet}
				</Accordion.Item>
			</Accordion>
		{/if}
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
