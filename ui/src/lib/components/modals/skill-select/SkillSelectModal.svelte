<script lang="ts">
	import { Card, Tooltip, Combobox } from '$components/ui';
	import type { ActiveSkill, PassiveSkill, SelectOption, SkillType } from '$types';
	import { Save, TimerReset, X, Delete } from 'lucide-svelte';
	import { activeSkillsData, passiveSkillsData, elementsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';

	let {
		title = '',
		value = $bindable(''),
		type = 'Active',
		palCharacterId = '',
		closeModal
	} = $props<{
		title?: string;
		value?: string;
		type?: SkillType;
		palCharacterId?: string;
		closeModal: (value: any) => void;
	}>();

	let selectOptions: SelectOption[] = $state([]);
	let activeSkills: ActiveSkill[] = $state([]);
	let passiveSkills: PassiveSkill[] = $state([]);
	let elementTypes: string[] = $state([]);
	let elementIcons: Record<string, string> = $state({});

	async function loadElementTypes() {
		elementTypes = await elementsData.getAllElementTypes();
	}

	async function loadElementIcons() {
		for (const elementType of elementTypes) {
			const elementObj = await elementsData.searchElement(elementType);
			if (elementObj) {
				const iconPath = `${ASSET_DATA_PATH}/img/elements/${elementObj.badge_icon}.png`;
				try {
					elementIcons[elementType] = await assetLoader.loadImage(iconPath, true);
				} catch (error) {
					console.error(`Failed to load icon for ${elementType}:`, error);
				}
			}
		}
	}

	async function getActiveSkills() {
		const allSkills = await activeSkillsData.getActiveSkills();
		const applicableSkills = allSkills.filter((skill) => {
			if (!skill.details.exclusive) {
				return true;
			}
			if (skill.details.exclusive.includes(palCharacterId)) {
				return true;
			}
			return false;
		});
		selectOptions = applicableSkills
			.sort((a, b) => a.details.type.localeCompare(b.details.type))
			.map((s) => ({
				value: s.id,
				label: s.name
			}));
		activeSkills = applicableSkills;
	}

	async function getPassiveSkills() {
		passiveSkills = await passiveSkillsData.getPassiveSkills();
		selectOptions = passiveSkills
			.sort((a, b) => a.details.tier.localeCompare(b.details.tier))
			.map((s) => ({
				value: s.id,
				label: s.name
			}));
	}

	function handleClear() {
		closeModal('Empty');
	}

	function handleClose(value: any) {
		closeModal(value);
	}

	async function getActiveSkillIcon(skillId: string): Promise<string | undefined> {
		const skill = activeSkills.find((s) => s.id === skillId);
		if (!skill || skill.name === 'None') return undefined;
		const activeSkill = skill as ActiveSkill;
		const elementObj = await elementsData.searchElement(activeSkill.details.type);
		if (!elementObj) return undefined;
		const iconPath = `${ASSET_DATA_PATH}/img/elements/${elementObj.icon}.png`;
		const icon = await assetLoader.loadImage(iconPath, true);
		return icon;
	}

	async function getPassiveSkillIcon(skillId: string): Promise<string | undefined> {
		const skill = passiveSkills.find((s) => s.id === skillId);
		if (!skill || skill.name === 'None') return undefined;
		const passiveSkill = skill as PassiveSkill;
		const iconPath = `${ASSET_DATA_PATH}/img/passives/Passive_${passiveSkill.details.tier.toUpperCase()}_icon.png`;
		const icon = await assetLoader.loadImage(iconPath, true);
		return icon;
	}

	$effect(() => {
		if (type === 'Active') {
			loadElementTypes();
		}
	});

	$effect(() => {
		if (elementTypes.length > 0 && type === 'Active') {
			loadElementIcons();
		}
	});

	$effect(() => {
		if (type === 'Active') {
			getActiveSkills();
		} else {
			getPassiveSkills();
		}
	});
</script>

{#snippet activeSkillOption(option: SelectOption)}
	{#await getActiveSkillIcon(option.value) then icon}
		{@const activeSkill = activeSkills.find((s) => s.id === option.value)}
		<div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
			{#if icon}
				<enhanced:img src={icon} alt={option.label} class="h-6 w-6"></enhanced:img>
			{:else}
				<div class="w-6"></div>
			{/if}
			<div class="flex flex-col">
				<span class="truncate">{option.label}</span>
				<span class="text-xs">{activeSkill?.description}</span>
			</div>
			<div class="flex items-center space-x-1 justify-self-start">
				<TimerReset class="h-4 w-4" />
				<span class="font-bold">{activeSkill?.details.ct}</span>
				<span class="text-xs">Pwr</span>
				<span class="font-bold">{activeSkill?.details.power}</span>
			</div>
		</div>
	{/await}
{/snippet}

{#snippet passiveSkillOption(option: SelectOption)}
	{#await getPassiveSkillIcon(option.value) then icon}
		{@const passiveSkill = passiveSkills.find((s) => s.id === option.value)}
		<div class="flex flex-row">
			<div class="flex grow flex-col">
				<span class="grow truncate">{option.label}</span>
				<span class="text-xs">{passiveSkill?.description}</span>
			</div>
			{#if icon}
				<enhanced:img src={icon} alt={option.label} class="h-6 w-6"></enhanced:img>
			{:else}
				<div class="w-6"></div>
			{/if}
		</div>
	{/await}
{/snippet}

<Card class="bg-surface-500 min-w-[calc(100vw/3)]">
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
