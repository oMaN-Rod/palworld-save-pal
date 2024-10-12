<script lang="ts">
	import { assetLoader } from '$lib/utils';
	import { passiveSkillsData } from '$lib/data';
	import type { PassiveSkill } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { SkillSelectModal } from '$components';
	import { getModalState } from '$states';
	import { Tooltip } from '$components/ui';

	let { skill, onSkillUpdate } = $props<{
		skill: string | null;
		onSkillUpdate: (newSkill: string, oldSkill: string) => void;
	}>();

	const modal = getModalState();

	let passiveSkill: PassiveSkill | null = $state(null);
	let tierIcon: string | null = $state(null);
	let sadIcon: string = $state('');

	$effect(() => {
		loadSkillData();
	});

	async function loadSkillData() {
		if (skill) {
			passiveSkill = await passiveSkillsData.searchPassiveSkills(skill);
			if (passiveSkill) {
				const iconPath = `${ASSET_DATA_PATH}/img/passives/Passive_${passiveSkill.details.tier.toUpperCase()}_icon.png`;
				tierIcon = await assetLoader.loadImage(iconPath, true);
			}
		}
		const sadIconPath = `${ASSET_DATA_PATH}/img/icons/Cattiva_Pleading.png`;
		sadIcon = await assetLoader.loadImage(sadIconPath, true);
	}

	async function handleSelectSkill() {
		// @ts-ignore
		const result = await modal.showModal<string>(SkillSelectModal, {
			type: 'Passive',
			value: skill,
			title: 'Select Passive Skill'
		});
		if (!result) return;
		onSkillUpdate(result, skill);
	}
</script>

{#snippet sad()}
	{#if sadIcon}
		<enhanced:img src={sadIcon} alt="Sad face icon" class="mr-2 h-6 w-6"></enhanced:img>
	{:else}
		<span class="mr-2">😞</span>
	{/if}
{/snippet}

{#snippet hasSkill(passiveSkill: PassiveSkill)}
	<Tooltip>
		<div class="flex w-full items-center">
			<span class="flex-grow p-2 text-start">{passiveSkill.name}</span>
			<div class="relative z-10 flex items-center p-2">
				{#if tierIcon}
					<enhanced:img src={tierIcon} alt="Passive skill tier icon" class="h-6 w-6 justify-start"
					></enhanced:img>
				{/if}
			</div>
		</div>
		{#snippet popup()}
			{#if passiveSkill?.description}
				{passiveSkill?.description}
			{:else}
				{@render sad()}
			{/if}
		{/snippet}
	</Tooltip>
{/snippet}

{#snippet noSkill()}
	<div class="flex w-full items-center">
		<span class="flex-grow p-2 text-start">
			{skill}
		</span>
		{#if skill === 'Empty'}
			{@render sad()}
		{:else}
			<span class="mr-2">❓</span>
		{/if}
	</div>
{/snippet}

<button
	class="hover:ring-secondary-500 border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none hover:ring"
	onclick={handleSelectSkill}
>
	{#if passiveSkill}
		{@render hasSkill(passiveSkill)}
	{:else}
		{@render noSkill()}
	{/if}
</button>
