<script lang="ts">
	import { passiveSkillsData } from '$lib/data';
	import { passiveSkillTier, type PassiveSkill } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$lib/constants';
	import { SkillSelectModal } from '$components';
	import { getModalState } from '$states';
	import { Tooltip } from '$components/ui';
	import { assetLoader } from '$utils';

	let { skill, onSkillUpdate } = $props<{
		skill: string | undefined;
		onSkillUpdate: (newSkill: string, oldSkill: string) => void;
	}>();

	const modal = getModalState();

	let skillData = $derived.by(() => {
		if (skill) {
			const passiveSkill = passiveSkillsData.passiveSkills[skill];
			if (!passiveSkill) {
				return null;
			}
			return passiveSkillsData.passiveSkills[skill];
		}
	});

	let tierIcon = $derived.by(() => {
		if (skillData) {
			const tier = passiveSkillTier(skillData.details.rank);
			return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/passives/Passive_${tier}_icon.png`);
		}
	});

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

{#snippet hasSkill(passiveSkill: PassiveSkill)}
	<Tooltip>
		<div class="flex w-full items-center">
			<span class="flex-grow p-2 text-start">{passiveSkill.localized_name}</span>
			<div class="relative z-10 flex items-center p-2">
				<img src={tierIcon} alt="Passive skill tier icon" class="h-6 w-6 justify-start" />
			</div>
		</div>
		{#snippet popup()}
			{#if passiveSkill.description}
				{passiveSkill.description}
			{:else}
				<img src={staticIcons.sadIcon} alt="Sad face icon" class="mr-2 h-6 w-6" />
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
			<img src={staticIcons.sadIcon} alt="Sad face icon" class="mr-2 h-6 w-6" />
		{:else}
			<span class="mr-2">❓</span>
		{/if}
	</div>
{/snippet}

<button
	class="hover:ring-secondary-500 border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none hover:ring"
	onclick={handleSelectSkill}
>
	{#if skillData}
		{@render hasSkill(skillData)}
	{:else}
		{@render noSkill()}
	{/if}
</button>
