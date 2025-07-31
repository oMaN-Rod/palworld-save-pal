<script lang="ts">
	import { Card, Tooltip, Combobox } from '$components/ui';
	import {
		type ActiveSkill,
		type Pal,
		type PassiveSkill,
		type SelectOption,
		type SkillType
	} from '$types';
	import { Save, X, Delete, TimerReset } from 'lucide-svelte';
	import { activeSkillsData, elementsData, passiveSkillsData } from '$lib/data';
	import { assetLoader, calculateFilters } from '$utils';
	import { ASSET_DATA_PATH, staticIcons } from '$types/icons';

	let {
		title = '',
		value = $bindable(''),
		type = 'Active',
		pal,
		closeModal
	} = $props<{
		title?: string;
		value?: string;
		type?: SkillType;
		pal?: Pal;
		closeModal: (value: any) => void;
	}>();

	const selectOptions: SelectOption[] = $derived.by(() => {
		let skills = [];
		if (type === 'Active') {
			skills = Object.values(activeSkillsData.activeSkills)
				.filter((skill) => {
					const characterKey = pal.character_key
						.toLowerCase()
						.replace('_dragon', '')
						.replace('_dark', '');
					const subString = `unique_${characterKey}`;
					if (skill.id.toLowerCase().includes(subString)) {
						return true;
					}
					if (!skill.id.toLowerCase().includes('unique_')) {
						return true;
					}
					return false;
				})
				.filter((aSkill) => !Object.values(pal.active_skills).some((skill) => skill === aSkill.id))
				.sort((a, b) => a.details.element.localeCompare(b.details.element))
				.map((s) => ({
					value: s.id,
					label: s.localized_name
				}));
		} else {
			skills = Object.values(passiveSkillsData.passiveSkills)
				.filter((pSkill) => !Object.values(pal.passive_skills).some((p) => p === pSkill.id))
				.sort((a, b) => b.details.rank - a.details.rank)
				.map((s) => ({
					value: s.id,
					label: s.localized_name
				}));
		}
		return skills;
	});

	function handleClear() {
		closeModal('Empty');
	}

	function handleClose(value: any) {
		closeModal(value);
	}

	function getActiveSkillIcon(skill: ActiveSkill | undefined) {
		if (!skill) return staticIcons.unknownIcon;
		const element = elementsData.getByKey(skill.details.element);
		if (!element) return undefined;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${element.icon}.webp`);
	}

	function getPassiveSkillFilter(skill: PassiveSkill | undefined) {
		if (!skill) return '';
		switch (skill?.details.rank) {
			case 1:
				return '';
			case 2:
			case 3:
				return calculateFilters('#fcdf19');
			case 4:
				return calculateFilters('#68ffd8');
			default:
				return calculateFilters('#FF0000');
		}
	}
</script>

{#snippet ActiveSkillOption(option: SelectOption)}
	{@const activeSkill = activeSkillsData.getByKey(option.value)}
	{@const icon = getActiveSkillIcon(activeSkill)}
	<div class="grid w-full grid-cols-[auto_1fr_auto] items-center gap-2">
		<img src={icon} alt={activeSkill?.localized_name} class="h-6 w-6" />
		<div class="mr-0.5 flex flex-col">
			<span class="truncate">{activeSkill?.localized_name}</span>
			<span class="text-xs">{activeSkill?.description}</span>
		</div>
		<div class="flex flex-col">
			<div class="flex items-center justify-end space-x-1">
				<TimerReset class="h-4 w-4" />
				<span class="font-bold">{activeSkill?.details.cool_time}</span>
				<span class="text-xs">Pwr</span>
				<span class="font-bold">{activeSkill?.details.power}</span>
			</div>
			<div class="flex flex-row items-center space-x-2">
				<div class="text-start">
					<span class="text-xs">{activeSkill?.details.type} Range</span>
					<span class="font-bold">
						{activeSkill?.details.min_range} - {activeSkill?.details.max_range}
					</span>
				</div>
			</div>
		</div>
	</div>
{/snippet}

{#snippet PassiveSkillOption(option: SelectOption)}
	{@const passiveSkill = passiveSkillsData.getByKey(option.value)}
	{@const icon = assetLoader.loadImage(
		`${ASSET_DATA_PATH}/img/rank_${passiveSkill?.details.rank}.webp`
	)}
	{@const filter = getPassiveSkillFilter(passiveSkill)}
	<div class="flex flex-row">
		<div class="flex grow flex-col">
			<span class="grow truncate">{option.label}</span>
			<span class="text-xs">{passiveSkill?.description}</span>
		</div>
		<img src={icon} alt={option.label} class="h-6 w-6" style="filter: {filter};" />
	</div>
{/snippet}

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<Combobox options={selectOptions} bind:value>
		{#snippet selectOption(option)}
			{#if type === 'Active'}
				{@render ActiveSkillOption(option)}
			{:else}
				{@render PassiveSkillOption(option)}
			{/if}
		{/snippet}
	</Combobox>

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
			<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(value)}>
				<Save />
			</button>
			{#snippet popup()}
				<span>Save</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(null)}>
				<X />
			</button>
			{#snippet popup()}
				<span>Cancel</span>
			{/snippet}
		</Tooltip>
	</div>
</Card>
