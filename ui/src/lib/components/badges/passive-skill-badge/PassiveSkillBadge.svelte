<script lang="ts">
	import { passiveSkillsData } from '$lib/data';
	import type { PassiveSkill } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$lib/constants';
	import { SkillSelectModal } from '$components';
	import { getModalState } from '$states';
	import { Tooltip } from '$components/ui';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';

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
			return assetLoader.loadImage(
				`${ASSET_DATA_PATH}/img/passives/rank_${skillData.details.rank}.png`
			);
		}
	});

	let backgroundImage = $derived.by(() => {
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/passives/bg.png`);
	});

	let borderClass = $derived.by(() => {
		if (skillData) {
			switch (skillData.details.rank) {
				case 2:
				case 3:
					return 'border-l-[#fcdf19]';
				case 4:
					return 'border-l-[#68ffd8]';
				default:
					return 'border-l-surface-600';
			}
		}
	});

	let bgOpacity = $derived.by(() => {
		if (skillData) {
			switch (skillData.details.rank) {
				case 1:
				case 2:
				case 3:
				case 4:
					return 'opacity-25';
			}
		}
		return 'opacity-0';
	});

	let filterStyle = $derived.by(() => {
		if (skillData) {
			switch (skillData.details.rank) {
				case 2:
				case 3:
					return calculateFilters('#fcdf19');
				case 4:
					return calculateFilters('#68ffd8');
				default:
					return '';
			}
		}
	});

	function hexToRGB(hex: string) {
		// Remove # if present
		hex = hex.replace('#', '');

		const r = parseInt(hex.substring(0, 2), 16) / 255;
		const g = parseInt(hex.substring(2, 4), 16) / 255;
		const b = parseInt(hex.substring(4, 6), 16) / 255;

		return { r, g, b };
	}

	// Calculate CSS filter values
	function calculateFilters(hex: string) {
		const rgb = hexToRGB(hex);

		// Matrix for color transformation
		const matrix = [
			rgb.r,
			0,
			0,
			0,
			0, // Red
			0,
			rgb.g,
			0,
			0,
			0, // Green
			0,
			0,
			rgb.b,
			0,
			0, // Blue
			0,
			0,
			0,
			1,
			0 // Alpha
		];

		return `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'><filter id='colorize'><feColorMatrix type='matrix' values='${matrix.join(' ')}'/></filter></svg>#colorize")`;
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

{#snippet hasSkill(passiveSkill: PassiveSkill)}
	<Tooltip>
		<div class="flex w-full items-center">
			<span class="flex-grow p-2 text-start">{passiveSkill.localized_name}</span>
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
