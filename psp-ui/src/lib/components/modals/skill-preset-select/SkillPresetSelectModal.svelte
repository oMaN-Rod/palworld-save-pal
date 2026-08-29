<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Combobox, Tooltip } from '$components/ui';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { activeSkillsData, elementsData, passiveSkillsData, presetsData } from '$lib/data';
	import { sortPresets } from '$states';
	import { cn } from '$theme';
	import { type PresetProfile, type SelectOption } from '$types';
	import { assetLoader, calculateFilters, deepCopy } from '$utils';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let {
		title = m.select_a_entity({ entity: c.preset }),
		type,
		closeModal
	} = $props<{
		title?: string;
		type: 'active' | 'passive';
		closeModal: (value: any) => void;
	}>();

	type ExtendedPresetProfile = PresetProfile & { id: string };

	const presetType = $derived(type === 'active' ? 'active_skills' : 'passive_skills');
	const backgroundImage = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/bg.webp`);

	const presets: ExtendedPresetProfile[] = $derived(
		sortPresets(
			Object.entries(presetsData.presetProfiles)
				.filter(([_, preset]) => preset.type === presetType)
				.map(([id, preset]) => ({ ...preset, id })),
			presetType
		)
	);
	const selectOptions: SelectOption[] = $derived(
		presets.map((preset) => ({ value: preset.id, label: preset.name }))
	);

	const elementIcons = $derived.by(() => {
		const icons: Record<string, string> = {};
		for (const [elementType, elementObj] of Object.entries(elementsData.elements)) {
			if (!elementObj) continue;
			icons[elementType] = assetLoader.loadImage(
				`${ASSET_DATA_PATH}/img/${elementObj.badge_icon}.webp`
			) as string;
		}
		return icons;
	});

	const passiveSkillIcons = $derived.by(() => {
		const icons: Record<string, string> = {};
		for (const skill of Object.values(passiveSkillsData.passiveSkills)) {
			if (icons[skill.details.rank]) continue;
			icons[skill.details.rank] = assetLoader.loadImage(
				`${ASSET_DATA_PATH}/img/rank_${skill.details.rank}.webp`
			) as string;
		}
		return icons;
	});

	let selectedPreset: string = $state('');

	function getPassiveSkillIconFilter(skillId: string): string {
		const skill = passiveSkillsData.getByKey(skillId);
		if (!skill || skill.localized_name === 'None') return '';
		switch (skill.details.rank) {
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
		const skill = passiveSkillsData.getByKey(skillId);
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

	function handleClose(confirmed: boolean) {
		if (!confirmed) {
			closeModal(undefined);
			return;
		}
		const preset = presets.find((p) => p.id === selectedPreset);
		closeModal(preset?.skills ? deepCopy(preset.skills) : undefined);
	}
</script>

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<Combobox options={selectOptions} bind:value={selectedPreset}>
		{#snippet selectOption(option)}
			{@const preset = presets.find((p) => p.id === option.value)}
			{#if preset && preset.skills}
				<div class="flex flex-col">
					<span>{option.label}</span>
					{#if type === 'active'}
						<div class="grid grid-cols-3 gap-2">
							{#each preset.skills as skill (skill)}
								{@const skillObj = activeSkillsData.getByKey(skill)}
								{#if skillObj}
									<div
										class="text-surface-400 border-surface-600 flex items-center space-x-1 rounded-xs border p-0.5"
									>
										<img
											src={elementIcons[skillObj.details.element]}
											alt={skillObj.details.element}
											class="h-4 w-4"
										/>
										<span class="grow text-xs">{skillObj.localized_name}</span>
										<span class="text-xs font-bold">{skillObj.details.power}</span>
									</div>
								{/if}
							{/each}
						</div>
					{:else}
						<div class="grid grid-cols-4 gap-2">
							{#each preset.skills as skill (skill)}
								{@const skillObj = passiveSkillsData.getByKey(skill)}
								{#if skillObj}
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
										<span class="grow text-xs">{skillObj.localized_name}</span>
										<img
											src={passiveSkillIcons[skillObj.details.rank]}
											alt={skillObj.details.rank.toString()}
											class="h-4 w-4"
											style="filter: {getPassiveSkillIconFilter(skill)};"
										/>
									</div>
								{/if}
							{/each}
						</div>
					{/if}
				</div>
			{/if}
		{/snippet}
	</Combobox>

	<div class="mt-2 flex flex-row items-center space-x-2">
		<Tooltip position="bottom">
			<Button
				variant="ghost"
				size="icon"
				disabled={!selectedPreset}
				onclick={() => handleClose(true)}
				data-modal-primary
			>
				<Icon icon="tabler:device-floppy" />
			</Button>
			{#snippet popup()}
				<span>{m.apply_selected_entity({ entity: c.preset })}</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<Button variant="ghost" size="icon" onclick={() => handleClose(false)}>
				<Icon icon="tabler:x" />
			</Button>
			{#snippet popup()}
				<span>{m.cancel()}</span>
			{/snippet}
		</Tooltip>
	</div>
</Card>
