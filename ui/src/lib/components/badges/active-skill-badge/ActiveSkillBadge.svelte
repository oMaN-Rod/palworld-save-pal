<script lang="ts">
	import { assetLoader } from '$lib/utils/asset-loader';
	import { activeSkillsData, elementsData } from '$lib/data';
	import type { ActiveSkill, Element } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { getModalState } from '$states';
	import { SkillSelectModal } from '$components/modals';
	import { Tooltip } from '$components/ui';
	import { TimerReset } from 'lucide-svelte';

	let {
		skill = 'Empty',
		palCharacterId,
		onSkillUpdate
	} = $props<{
		skill: string;
		palCharacterId: string;
		onSkillUpdate: (newSkill: string, oldSkill: string) => void;
	}>();

	const modal = getModalState();

	let activeSkill: ActiveSkill | null = $state(null);
	let element: Element | undefined = $state(undefined);
	let elementIcon: string | null = $state(null);

	$effect(() => {
		loadSkillData();
	});

	async function loadSkillData() {
		activeSkill = await activeSkillsData.searchActiveSkills(skill);
		if (activeSkill) {
			element = await elementsData.searchElement(activeSkill.details.type);
			if (element) {
				const iconPath = `${ASSET_DATA_PATH}/img/elements/${element.white_icon}.png`;
				elementIcon = await assetLoader.loadImage(iconPath, true);
			}
		} else {
			console.log('No active skill found for:', skill);
		}
	}

	async function handleSelectSkill() {
		// @ts-ignore
		const result = await modal.showModal<string>(SkillSelectModal, {
			type: 'Active',
			value: skill,
			title: 'Select Active Skill',
			palCharacterId
		});
		if (!result) return;
		onSkillUpdate(result, skill);
	}
</script>

{#snippet hasSkill()}
	<Tooltip>
		<div
			class="border-l-primary bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
			style="border-left-color: {element?.color}"
		>
			<div class="flex w-full">
				<span class="flex-grow p-2 text-left text-lg">{activeSkill?.name}</span>
				<div
					class="text-on-surface relative z-10 flex items-center p-2"
					style="background-color: {element?.color}"
				>
					{#if elementIcon}
						<enhanced:img src={elementIcon} alt="{element?.name} icon" class="h-6 w-6 justify-start"
						></enhanced:img>
					{/if}
					<span class="ml-2 text-lg font-bold"
						>{activeSkill?.details.power == 0 ? 'NA' : activeSkill?.details.power}</span
					>
				</div>
			</div>
			<div
				class="absolute bottom-0 right-[3rem] top-0 w-8 origin-top-right skew-x-[-20deg] transform"
				style="background-color: {element?.color}"
			></div>
		</div>
		{#snippet popup()}
			<div class="flex flex-row p-2">
				<span>{activeSkill?.description}</span>
				<div class="ml-4 flex flex-row">
					<TimerReset class="h-6 w-6" />
					{activeSkill?.details.ct}
				</div>
			</div>
		{/snippet}
	</Tooltip>
{/snippet}

{#snippet noSkill()}
	<div
		class="border-l-primary border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
	>
		<div class="flex w-full items-center">
			<span class="flex-grow p-2 text-lg">{skill}</span>
		</div>
	</div>
{/snippet}

<button class="hover:ring-secondary-500 hover:ring" onclick={handleSelectSkill}>
	{#if activeSkill && element}
		{@render hasSkill()}
	{:else}
		{@render noSkill()}
	{/if}
</button>
