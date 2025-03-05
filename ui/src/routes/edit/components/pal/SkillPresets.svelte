<script lang="ts">
	import { Combobox, TooltipButton } from '$components/ui';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { elementsData, presetsData, activeSkillsData, passiveSkillsData } from '$lib/data';
	import { getModalState } from '$states';
	import { cn } from '$theme';
	import { type PassiveSkill, type PresetProfile, type SelectOption } from '$types';
	import { assetLoader, calculateFilters, deepCopy } from '$utils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { Play, Trash } from 'lucide-svelte';

	let { onSelect } = $props<{
		onSelect: (type: 'active' | 'passive', value: string[]) => void;
	}>();

	const modal = getModalState();
	const backgroundImage = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/bg.png`);
	let selected: string[] = $state(['passive']);

	type ExtendedPresetProfile = PresetProfile & { id: string };

	let activeSkillPresets: ExtendedPresetProfile[] = $derived.by(() => {
		return Object.entries(presetsData.presetProfiles)
			.filter(([_, preset]) => preset.type === 'active_skills')
			.map(([id, preset]) => ({ ...preset, id }));
	});
	let activeSkillPresetOptions: SelectOption[] = $derived.by(() => {
		return activeSkillPresets.map((preset) => ({
			value: preset.id,
			label: preset.name
		}));
	});
	let selectedActiveSkillPreset: string = $state('');

	let passiveSkillPresets: ExtendedPresetProfile[] = $derived.by(() => {
		return Object.entries(presetsData.presetProfiles)
			.filter(([_, preset]) => preset.type === 'passive_skills')
			.map(([id, preset]) => ({ ...preset, id }));
	});
	let passiveSkillPresetOptions: SelectOption[] = $derived.by(() => {
		return passiveSkillPresets.map((preset) => ({
			value: preset.id,
			label: preset.name
		}));
	});
	let selectedPassiveSkillPreset: string = $state('');

	let elementIcons = $derived.by(() => {
		const icons: Record<string, string> = {};
		for (const elementType of Object.keys(elementsData.elements)) {
			const elementObj = elementsData.elements[elementType];
			if (elementObj) {
				icons[elementType] = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/${elementObj.badge_icon}.png`
				) as string;
			}
		}
		return icons;
	});
	let passiveSkillIcons = $derived.by(() => {
		const icons: Record<string, string> = {};
		for (const skill of Object.values(passiveSkillsData.passiveSkills)) {
			if (icons[skill.details.rank]) continue;
			icons[skill.details.rank] = assetLoader.loadImage(
				`${ASSET_DATA_PATH}/img/rank_${skill.details.rank}.png`
			) as string;
		}
		return icons;
	});

	async function handleApplyPreset(type: 'active' | 'passive') {
		const preset =
			type === 'active'
				? activeSkillPresets.find((p) => p.id === selectedActiveSkillPreset)
				: passiveSkillPresets.find((p) => p.id === selectedPassiveSkillPreset);
		if (!preset) return;
		onSelect(type, deepCopy(preset.skills));
	}

	async function handleDeletePreset(type: 'active' | 'passive') {
		const presetId = type === 'active' ? selectedActiveSkillPreset : selectedPassiveSkillPreset;
		if (!presetId) return;
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Preset',
			message: `Are you sure you want to delete the selected preset?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (!confirmed) return;
		await presetsData.removePresetProfiles([presetId]);
	}

	function getPassiveSkillIconFilter(skillId: string): string {
		const skill = passiveSkillsData.passiveSkills[skillId];
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

	function getPassiveSkillBorderClass(skillId: string): string {
		const skill = passiveSkillsData.passiveSkills[skillId];
		if (!skill) return '';
		switch (skill.details.rank) {
			case 1:
				return 'border-l-surface-600';
			case 2:
			case 3:
				return 'border-l-[#fcdf19]';
			case 4:
				return 'border-l-[#68ffd8]';
			default:
				return 'border-l-[#FF0000]';
		}
	}
</script>

<Accordion value={selected} onValueChange={(e) => (selected = e.value)} collapsible>
	<Accordion.Item value="active" controlHover="hover:preset-tonal-secondary">
		{#snippet control()}
			Active Skills
		{/snippet}
		{#snippet panel()}
			<div class="flex flex-col">
				<Combobox
					options={activeSkillPresetOptions}
					bind:value={selectedActiveSkillPreset}
					showHorizontalRule
				>
					{#snippet selectOption(option)}
						{@const preset = activeSkillPresets.find((p) => p.id === option.value)}
						{#if preset && preset.skills}
							<div class="flex flex-col">
								<span>{option.label}</span>
								<div class="grid grid-cols-3 gap-2">
									{#each preset.skills as skill}
										{@const skillObj = Object.values(activeSkillsData.activeSkills).find(
											(s) => s.id === skill
										)}
										{#if skillObj}
											{@const icon = elementIcons[skillObj.details.element]}
											<div
												class="text-surface-400 border-surface-600 r rounded-xs flex items-center space-x-1 border p-0.5"
											>
												<img src={icon} alt={skillObj.details.element} class="h-4 w-4" />
												<span class="grow text-xs">
													{skillObj.localized_name}
												</span>
												<span class=" text-xs font-bold">
													{skillObj.details.power}
												</span>
											</div>
										{/if}
									{/each}
								</div>
							</div>
						{/if}
					{/snippet}
				</Combobox>
				<div class="flex justify-end">
					<TooltipButton
						popupLabel="Apply selected preset"
						onclick={() => handleApplyPreset('active')}
						disabled={!selectedActiveSkillPreset}
					>
						<Play class="text-primary-500" size="24" />
					</TooltipButton>
					<TooltipButton
						popupLabel="Delete selected preset"
						onclick={() => handleDeletePreset('active')}
						disabled={!selectedActiveSkillPreset}
					>
						<Trash class="text-primary-500" size="24" />
					</TooltipButton>
				</div>
			</div>
		{/snippet}
	</Accordion.Item>
	<Accordion.Item value="passive" controlHover="hover:preset-tonal-secondary">
		{#snippet control()}
			Passive Skills
		{/snippet}
		{#snippet panel()}
			<div class="flex flex-col">
				<Combobox options={passiveSkillPresetOptions} bind:value={selectedPassiveSkillPreset}>
					{#snippet selectOption(option)}
						{@const preset = passiveSkillPresets.find((p) => p.id === option.value)}
						{#if preset && preset.skills}
							<div class="flex flex-col">
								<span>{option.label}</span>
								<div class="grid grid-cols-4 gap-2">
									{#each preset.skills as skill}
										{@const skillObj = Object.values(passiveSkillsData.passiveSkills).find(
											(s) => s.id === skill
										)}
										{#if skillObj}
											{@const icon = passiveSkillIcons[skillObj.details.rank]}
											<div
												class={cn(
													'relative flex items-center space-x-1 border-l-2 p-0.5',
													getPassiveSkillBorderClass(skill)
												)}
											>
												<div
													class="absolute inset-0 bg-cover bg-center opacity-25"
													style="background-image: url('{backgroundImage}'); filter: {getPassiveSkillIconFilter(
														skill
													)};"
												></div>
												<span class="grow text-xs">
													{skillObj.localized_name}
												</span>
												<img
													src={icon}
													alt={skillObj.details.rank.toString()}
													class="h-4 w-4"
													style="filter: {getPassiveSkillIconFilter(skill)};"
												/>
											</div>
										{/if}
									{/each}
								</div>
							</div>
						{/if}
					{/snippet}
				</Combobox>
				<div class="flex justify-end">
					<TooltipButton
						popupLabel="Apply selected preset"
						onclick={() => handleApplyPreset('passive')}
						disabled={!selectedPassiveSkillPreset}
					>
						<Play class="text-primary-500" size="24" />
					</TooltipButton>
					<TooltipButton
						popupLabel="Delete selected preset"
						onclick={() => handleDeletePreset('passive')}
						disabled={!selectedPassiveSkillPreset}
					>
						<Trash class="text-primary-500" size="24" />
					</TooltipButton>
				</div>
			</div>
		{/snippet}
	</Accordion.Item>
</Accordion>
