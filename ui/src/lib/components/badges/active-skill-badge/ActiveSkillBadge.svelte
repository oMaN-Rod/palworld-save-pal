<script lang="ts">
	import { activeSkillsData, elementsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { getAppState, getModalState } from '$states';
	import { SkillSelectModal } from '$components/modals';
	import { Tooltip } from '$components/ui';
	import { TimerReset } from 'lucide-svelte';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let { skill = 'Empty', onSkillUpdate } = $props<{
		skill: string;
		onSkillUpdate?: (newSkill: string, oldSkill: string) => void;
	}>();

	const appState = getAppState();
	const modal = getModalState();

	let { activeSkill, element, elementIconWhite, elementIcon } = $derived.by(() => {
		if (skill === 'Empty') return {};
		const activeSkill = activeSkillsData.getByKey(skill) || undefined;
		if (!activeSkill) {
			console.error(`Active skill ${skill} not found`);
			return {};
		}
		const element = elementsData.getByKey(activeSkill.details.element);
		const elementIconWhite = assetLoader.loadImage(
			`${ASSET_DATA_PATH}/img/${element?.white_icon}.webp`
		);
		const elementIcon = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${element?.icon}.webp`);
		return { activeSkill, element, elementIconWhite, elementIcon };
	});

	async function handleSelectSkill() {
		// @ts-ignore
		const result = await modal.showModal<string>(SkillSelectModal, {
			type: 'Active',
			value: skill,
			title: m.select_entity({ entity: c.activeSkill }),
			pal: appState.selectedPal
		});
		if (!result) return;
		onSkillUpdate(result, skill);
	}
</script>

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
				<span class="grow p-2 text-left text-sm 2xl:text-base">{activeSkill?.localized_name}</span>
				<div
					class="text-on-surface relative z-10 flex items-center p-2"
					style="background-color: {element?.color}"
				>
					{#if elementIconWhite}
						<img
							src={elementIconWhite}
							alt="{element?.localized_name} icon"
							class="h-6 w-6 justify-start"
						/>
					{/if}
					<span class="ml-2 text-sm font-bold 2xl:text-base"
						>{activeSkill?.details.power == 0 ? 'NA' : activeSkill?.details.power}</span
					>
				</div>
			</div>
			<div
				class="absolute bottom-0 right-12 top-0 w-8 origin-top-right skew-x-[-20deg] transform"
				style="background-color: {element?.color}"
			></div>
		</div>
		{#snippet popup()}
			<div class="bg-surface-800 flex w-96 flex-col">
				<div class="flex flex-col space-y-2 border-b p-2">
					<h4 class="h4 text-left">{activeSkill?.localized_name}</h4>
					<div class="grid grid-cols-[1fr_auto] gap-2">
						<span class="grow text-left text-gray-300">
							<div class="flex">
								<img src={elementIcon} alt="{element?.localized_name} icon" class="h-6 w-6" />
								{activeSkill?.details.element}
							</div>
						</span>
						<div class="flex items-center space-x-2">
							<TimerReset class="h-4 w-4" />
							<span class="font-bold">{activeSkill?.details.cool_time}</span>
							<span class="text-xs">{m.pwr()}</span>
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
							<span class="text-xs">{m.range()}</span>
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
				<img src={staticIcons.sadIcon} alt="Sad face icon" class="mr-2 h-6 w-6" />
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
