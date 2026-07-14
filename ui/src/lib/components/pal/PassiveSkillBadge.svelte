<script lang="ts">
	import { passiveSkillsData } from '$lib/data';
	import type { Pal, PassiveSkill } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { SkillSelectModal } from '$components/modals';
	import { getModalState } from '$states';
	import { Tooltip } from '$components/ui';
	import { assetLoader, calculateFilters, skillBorderClass, skillFilter, skillOpacity } from '$utils';
	import { cn } from '$theme';
	import { staticIcons } from '$types/icons';
	import { HelpCircle } from 'lucide-svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let { skill, pal, onSkillUpdate } = $props<{
		skill: string | undefined;
		pal?: Pal;
		onSkillUpdate?: (newSkill: string, oldSkill: string) => void;
	}>();
	const modal = getModalState();

	let skillData = $derived(passiveSkillsData.getByKey(skill));

	let tierIcon = $derived.by(() => {
		if (skillData) {
			return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/rank_${skillData.details.rank}.webp`);
		}
	});

	const backgroundImage = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/bg.webp`);

	let borderClass = $derived.by(() => {
		if (skillData) {
			return skillBorderClass(skillData.details.rank);
		}
	});

	let bgOpacity = $derived.by(() => {
		if (skillData) {
			return skillOpacity(skillData.details.rank);
		}
		return 'opacity-0';
	});

	let filterStyle = $derived.by(() => {
		if (skillData) {
			return skillFilter(skillData.details.rank);
		}
	});

	// Calculate CSS filter values

	async function handleSelectSkill() {
		// @ts-ignore
		const result = await modal.showModal<string>(SkillSelectModal, {
			type: 'Passive',
			value: skill,
			title: m.select_entity({ entity: c.passiveSkill }),
			pal
		});
		if (!result) return;
		onSkillUpdate(result, skill);
	}
</script>

{#snippet hasSkill(passiveSkill: PassiveSkill)}
	<Tooltip>
		<div class="flex w-full items-center">
			<span class="grow p-2 text-start text-sm 2xl:text-base">{passiveSkill.localized_name}</span>
			<div class="relative z-10 flex items-center p-2">
				<img
					src={tierIcon}
					alt="Passive skill tier icon"
					class="h-6 w-6 justify-start"
					style="filter: {filterStyle};"
				/>
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
		<span class="grow p-2 text-start">
			{skill}
		</span>
		{#if skill === 'Empty'}
			<img src={staticIcons.sadIcon} alt="Sad face icon" class="mr-2 h-6 w-6" />
		{:else}
			<HelpCircle size={18} class="text-surface-500 mr-2" />
		{/if}
	</div>
{/snippet}

<button
	class={cn(
		'hover:ring-secondary-500 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none hover:ring',
		borderClass
	)}
	onclick={handleSelectSkill}
>
	<div
		class={cn('absolute inset-0 bg-cover bg-center', bgOpacity)}
		style="background-image: url('{backgroundImage}'); filter: {filterStyle};"
	></div>
	<div class="relative">
		{#if skillData}
			{@render hasSkill(skillData)}
		{:else}
			{@render noSkill()}
		{/if}
	</div>
</button>
