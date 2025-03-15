<script lang="ts">
	import { Card, Tooltip, Combobox } from '$components/ui';
	import {
		type ActiveSkill,
		type Pal,
		type PassiveSkill,
		type SelectOption,
		type SkillType
	} from '$types';
	import { Save, TimerReset, X, Delete } from 'lucide-svelte';
	import { activeSkillsData, passiveSkillsData, elementsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader, calculateFilters } from '$utils';

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

	let selectOptions: SelectOption[] = $derived.by(() => {
		if (type === 'Active') {
			return Object.values(activeSkillsData.activeSkills)
				.filter((skill) => {
					if (skill.id.toLowerCase().includes(`unique_${pal.character_id.toLowerCase()}`)) {
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
			return Object.values(passiveSkillsData.passiveSkills)
				.filter((pSkill) => !Object.values(pal.passive_skills).some((p) => p === pSkill.id))
				.sort((a, b) => b.details.rank - a.details.rank)
				.map((s) => ({
					value: s.id,
					label: s.localized_name
				}));
		}
	});

	function handleClear() {
		closeModal('Empty');
	}

	function handleClose(value: any) {
		closeModal(value);
	}

	async function getActiveSkillIcon(skillId: string): Promise<string | undefined> {
		const skill = Object.values(activeSkillsData.activeSkills).find((s) => s.id === skillId);
		if (!skill || skill.localized_name === 'None') return undefined;
		const activeSkill = skill as ActiveSkill;
		const element = await elementsData.searchElement(activeSkill.details.element);
		if (!element) return undefined;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${element.icon}.png`);
	}

	async function getPassiveSkillIcon(skillId: string): Promise<string | undefined> {
		const skill = Object.values(passiveSkillsData.passiveSkills).find((s) => s.id === skillId);
		if (!skill || skill.localized_name === 'None') return undefined;
		const passiveSkill = skill as PassiveSkill;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/rank_${passiveSkill.details.rank}.png`);
	}

	function getPassiveSkillIconFilter(skillId: string): string {
		const skill = Object.values(passiveSkillsData.passiveSkills).find((s) => s.id === skillId);
		if (!skill || skill.localized_name === 'None') return '';
		const passiveSkill = skill as PassiveSkill;
		switch (passiveSkill.details.rank) {
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

{#snippet activeSkillOption(option: SelectOption)}
	{#await getActiveSkillIcon(option.value) then icon}
		{@const activeSkill = Object.values(activeSkillsData.activeSkills).find(
			(s) => s.id === option.value
		)}
		<div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
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
						<span class="font-bold"
							>{activeSkill?.details.min_range} - {activeSkill?.details.max_range}</span
						>
					</div>
				</div>
			</div>
		</div>
	{/await}
{/snippet}

{#snippet passiveSkillOption(option: SelectOption)}
	{#await getPassiveSkillIcon(option.value) then icon}
		{@const passiveSkill = Object.values(passiveSkillsData.passiveSkills).find(
			(s) => s.id === option.value
		)}
		<div class="flex flex-row">
			<div class="flex grow flex-col">
				<span class="grow truncate">{option.label}</span>
				<span class="text-xs">{passiveSkill?.description}</span>
			</div>
			<img
				src={icon}
				alt={option.label}
				class="h-6 w-6"
				style="filter: {getPassiveSkillIconFilter(option.value)};"
			/>
		</div>
	{/await}
{/snippet}

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<Combobox options={selectOptions} bind:value>
		{#snippet selectOption(option)}
			{#if type === 'Active'}
				{@render activeSkillOption(option)}
			{:else}
				{@render passiveSkillOption(option)}
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
