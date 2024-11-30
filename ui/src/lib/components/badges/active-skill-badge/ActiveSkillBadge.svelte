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
	let elementIconWhite: string | null = $state(null);
	let sadIcon: string = $state('');

	$effect(() => {
		loadSkillData();
	});

	async function loadSkillData() {
		activeSkill = await activeSkillsData.searchActiveSkills(skill);
		if (activeSkill) {
			element = await elementsData.searchElement(activeSkill.details.element);
			if (element) {
				const whiteIconPath = `${ASSET_DATA_PATH}/img/elements/${element.white_icon}.png`;
				elementIconWhite = await assetLoader.loadImage(whiteIconPath, true);
				const iconPath = `${ASSET_DATA_PATH}/img/elements/${element.icon}.png`;
				elementIcon = await assetLoader.loadImage(iconPath, true);
			}
		} else {
			console.log('No active skill found for:', skill);
		}
		const sadIconPath = `${ASSET_DATA_PATH}/img/icons/Cattiva_Pleading.png`;
		sadIcon = await assetLoader.loadImage(sadIconPath, true);
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

{#snippet sad()}
	{#if sadIcon}
		<enhanced:img src={sadIcon} alt="Sad face icon" class="mr-2 h-6 w-6"></enhanced:img>
	{:else}
		<span class="mr-2">😞</span>
	{/if}
{/snippet}

{#snippet hasSkill()}
	<Tooltip
		popupClass="p-0 mt-12 bg-surface-600"
		rounded="rounded-none"
		position="right"
		useArrow={false}
	>
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
					{#if elementIconWhite}
						<enhanced:img
							src={elementIconWhite}
							alt="{element?.name} icon"
							class="h-6 w-6 justify-start"
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
			<div class="bg-surface-800 flex w-96 flex-col">
				<div class="flex flex-col space-y-2 border-b p-2">
					<h4 class="h4 text-left">{activeSkill?.name}</h4>
					<div class="grid grid-cols-[1fr_auto] gap-2">
						<span class="grow text-left text-gray-300">
							{#if elementIcon}
								<div class="flex">
									<enhanced:img src={elementIcon} alt="{element?.name} icon" class="h-6 w-6"
									></enhanced:img>
									{activeSkill?.details.element}
								</div>
							{:else}
								{activeSkill?.details.element}
							{/if}
						</span>
						<div class="flex items-center space-x-2">
							<TimerReset class="h-4 w-4" />
							<span class="font-bold">{activeSkill?.details.ct}</span>
							<span class="text-xs">Pwr</span>
							<span class="font-bold">{activeSkill?.details.power}</span>
						</div>
					</div>
				</div>
				<div class="bg-surface-600 p-2 text-left">
					<span class="whitespace-pre-line">{activeSkill?.description}</span>
				</div>
				<div>
					<div class="flex flex-row items-center space-x-2 p-2">
						<div class="grow text-start">
							<span class="text-xs">Range</span>
							<span class="font-bold"
								>{activeSkill?.details.min_range} - {activeSkill?.details.max_range}</span
							>
						</div>
						<div class="border-l border-r p-2 px-2 py-0.5 text-left text-sm font-bold">
							{activeSkill?.details.type}
						</div>
					</div>
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
			<span class="grow p-2 text-start text-lg">{skill}</span>
			{#if skill === 'Empty'}
				{@render sad()}
			{:else}
				<span class="mr-2">❓</span>
			{/if}
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
