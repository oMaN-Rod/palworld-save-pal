<script lang="ts">
	import { PalHeader, SkillSelectModal } from '$components';
	import StatusBadge from '$components/badges/status-badge/StatusBadge.svelte';
	import {
		ActiveSkillBadge,
		PassiveSkillBadge,
		StatsBadges,
		WorkSuitabilities,
		TextInputModal,
		Talents,
		LearnedSkillSelectModal
	} from '$components';
	import { SectionHeader, Tooltip } from '$components/ui';
	import { EntryState, type PresetProfile, type WorkSuitability } from '$types';
	import { staticIcons } from '$types/icons';
	import { palsData, expData, presetsData } from '$lib/data';
	import { getAppState, getModalState } from '$states';
	import { BicepsFlexed, Brain, Plus, Save } from 'lucide-svelte';
	import { Souls } from '$components';
	import SkillPresets from './SkillPresets.svelte';
	import { assetLoader, calculateFilters } from '$utils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import MultiSkillSelectModal from '$components/modals/multi-skill-select/MultiSkillSelectModal.svelte';
	import type { ValueChangeDetails } from '@zag-js/accordion';

	const appState = getAppState();

	const modal = getModalState();

	let palLevelProgressToNext: number = $state(0);
	let palLevelProgressValue: number = $state(0);
	let palLevelProgressMax: number = $state(1);
	let leftAccordionValue: string[] = $state(['active_skills']);
	let rightAccordionValue: string[] = $state(['stats']);

	const max_talent = $derived(appState.settings.cheat_mode ? 255 : 100);
	const max_souls = $derived(appState.settings.cheat_mode ? 255 : 20);

	const palImage = $derived.by(() => {
		if (appState.selectedPal) {
			const { character_key } = appState.selectedPal;
			const palData = palsData.getByKey(character_key);
			return assetLoader.loadPalImage(character_key, palData?.is_pal || false);
		}
	});

	const activeSkills = $derived.by(() => {
		if (appState.selectedPal) {
			let skills = [...appState.selectedPal.active_skills];
			while (skills.length < 3) {
				skills.push('Empty');
			}
			return skills;
		} else {
			return [];
		}
	});

	const passiveSkills = $derived.by(() => {
		if (appState.selectedPal) {
			let skills = [...appState.selectedPal.passive_skills];
			while (skills.length < 4) {
				skills.push('Empty');
			}
			return skills;
		} else {
			return [];
		}
	});

	async function calcPalLevelProgress() {
		if (appState.selectedPal) {
			if (appState.selectedPal.level === 60) {
				palLevelProgressToNext = 0;
				palLevelProgressValue = 0;
				palLevelProgressMax = 1;
				return;
			}
			const nextExp = await expData.getExpDataByLevel(appState.selectedPal.level + 1);
			palLevelProgressToNext = nextExp.PalTotalEXP - appState.selectedPal.exp;
			palLevelProgressValue = nextExp.PalNextEXP - palLevelProgressToNext;
			palLevelProgressMax = nextExp.PalNextEXP;
		}
	}

	async function getPalDescription(character_id: string): Promise<string | undefined> {
		const palData = palsData.getByKey(character_id);
		if (!palData) return undefined;
		return palData.description;
	}

	async function handleEditLearnedSkills() {
		if (!appState.selectedPal) return;
		// @ts-ignore
		const result = await modal.showModal<string[]>(LearnedSkillSelectModal, {
			pal: appState.selectedPal
		});
		if (result) {
			appState.selectedPal.learned_skills = result;
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	function handleUpdateActiveSkill(newSkill: string, oldSkill: string): void {
		if (appState.selectedPal) {
			const targetSkillIndex = appState.selectedPal.active_skills.findIndex((s) => s === oldSkill);

			if (newSkill === 'Empty') {
				if (targetSkillIndex >= 0) {
					appState.selectedPal.active_skills.splice(targetSkillIndex, 1);
				}
			} else {
				if (targetSkillIndex >= 0) {
					appState.selectedPal.active_skills[targetSkillIndex] = newSkill;
				} else {
					appState.selectedPal.active_skills.push(newSkill);
				}
			}

			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	function handleUpdatePassiveSkill(newSkill: string, oldSkill: string): void {
		if (appState.selectedPal) {
			const targetSkillIndex = appState.selectedPal.passive_skills.findIndex((s) => s === oldSkill);

			if (newSkill === 'Empty') {
				if (targetSkillIndex >= 0) {
					appState.selectedPal.passive_skills.splice(targetSkillIndex, 1);
				}
			} else {
				if (targetSkillIndex >= 0) {
					appState.selectedPal.passive_skills[targetSkillIndex] = newSkill;
				} else {
					appState.selectedPal.passive_skills.push(newSkill);
				}
			}

			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	async function setSkillPreset(type: 'active' | 'passive', skills: string[]) {
		if (appState.selectedPal) {
			if (type === 'active') {
				appState.selectedPal.active_skills = skills || [];
			} else {
				appState.selectedPal.passive_skills = skills || [];
			}
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	async function handleAddPreset(type: 'active' | 'passive') {
		if (!appState.selectedPal) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: `Add ${type} skills preset`,
			value: '',
			inputLabel: 'Preset name'
		});
		if (!result) return;
		const skills =
			type === 'active' ? appState.selectedPal.active_skills : appState.selectedPal.passive_skills;
		const newPreset = {
			name: result,
			type: type === 'active' ? 'active_skills' : 'passive_skills',
			skills
		} as PresetProfile;

		await presetsData.addPresetProfile(newPreset);
	}

	function handleMaxIVs() {
		if (appState.selectedPal) {
			appState.selectedPal.talent_hp = max_talent;
			appState.selectedPal.talent_shot = max_talent;
			appState.selectedPal.talent_defense = max_talent;
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	function handleMaxSouls() {
		if (appState.selectedPal) {
			appState.selectedPal.rank_hp = max_souls;
			appState.selectedPal.rank_attack = max_souls;
			appState.selectedPal.rank_defense = max_souls;
			appState.selectedPal.rank_craftspeed = max_souls;
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	function handleMaxWorkSuitability() {
		if (!appState.selectedPal) return;
		const palData = palsData.getByKey(appState.selectedPal.character_key);
		if (!palData) return;
		for (const [key, value] of Object.entries(palData.work_suitability)) {
			if (value === 0) continue;
			appState.selectedPal.work_suitability[key as WorkSuitability] = Math.min(5 - value, 4);
		}
		appState.selectedPal.state = EntryState.MODIFIED;
	}

	async function handleAddSkill(type: 'active' | 'passive') {
		// @ts-ignore
		const result = await modal.showModal<string[]>(MultiSkillSelectModal, {
			type: type === 'active' ? 'Active' : 'Passive',
			title: `Select ${type === 'active' ? 'Active' : 'Passive'} Skill`,
			pal: appState.selectedPal
		});
		if (!result) return;
		if (type === 'active') {
			appState.selectedPal!.active_skills.push(...result);
		} else {
			appState.selectedPal!.passive_skills.push(...result);
		}
	}

	$effect(() => {
		calcPalLevelProgress();
	});
</script>

{#snippet activeSkillsHeader()}
	<SectionHeader text="Active Skills">
		{#snippet action()}
			<div class="flex">
				<Tooltip label="Edit Learned Skills">
					<button
						class="btn hover:bg-secondary-500/25 ml-2 p-2"
						onclick={(event) => {
							event.stopPropagation();
							handleEditLearnedSkills();
						}}
					>
						<Brain size={20} />
					</button>
				</Tooltip>
				<Tooltip label="Save as preset">
					<button
						class="btn hover:bg-secondary-500/25 ml-2 p-2"
						onclick={(event) => {
							event.stopPropagation();
							handleAddPreset('active');
						}}
					>
						<Save size={20} />
					</button>
				</Tooltip>
				<Tooltip label="Add Active Skills">
					<button
						class="btn hover:bg-secondary-500/25 ml-2 p-2"
						onclick={(event) => {
							event.stopPropagation();
							handleAddSkill('active');
						}}
					>
						<Plus size={20} />
					</button>
				</Tooltip>
			</div>
		{/snippet}
	</SectionHeader>
{/snippet}

{#snippet activeSkillsBody()}
	<div class="flex max-h-36 flex-col space-y-2 overflow-y-auto">
		{#each activeSkills as skill}
			<ActiveSkillBadge {skill} onSkillUpdate={handleUpdateActiveSkill} />
		{/each}
	</div>
{/snippet}

{#snippet passiveSkillsHeader()}
	<SectionHeader text="Passive Skills">
		{#snippet action()}
			<div class="flex">
				<Tooltip label="Save as preset">
					<button
						class="btn hover:bg-secondary-500/25 ml-2 p-2"
						onclick={(event) => {
							event.stopPropagation();
							handleAddPreset('passive');
						}}
					>
						<Save size={20} />
					</button>
				</Tooltip>
				<Tooltip label="Add Passive Skill">
					<button
						class="btn hover:bg-secondary-500/25 ml-2 p-2"
						onclick={(event) => {
							event.stopPropagation();
							handleAddSkill('passive');
						}}
					>
						<Plus size={20} />
					</button>
				</Tooltip>
			</div>
		{/snippet}
	</SectionHeader>
{/snippet}

{#snippet passiveSkillsBody()}
	<div class="grid max-h-24 grid-cols-2 gap-2 overflow-y-auto">
		{#each passiveSkills as skill}
			<PassiveSkillBadge {skill} onSkillUpdate={handleUpdatePassiveSkill} />
		{/each}
	</div>
{/snippet}

{#snippet workSuitabilityHeader()}
	<SectionHeader text="Work Suitability">
		{#snippet action()}
			<div class="flex">
				<Tooltip label="Max out Work Suitability">
					<button
						class="btn hover:bg-secondary-500/25 ml-2 p-2"
						onclick={(event) => {
							event.stopPropagation();
							handleMaxWorkSuitability();
						}}
					>
						<BicepsFlexed />
					</button>
				</Tooltip>
			</div>
		{/snippet}
	</SectionHeader>
{/snippet}

{#snippet talentsHeader()}
	<SectionHeader text="Talents (IVs)">
		{#snippet action()}
			<div class="flex">
				<Tooltip label="Max out IVs">
					<button
						class="btn hover:bg-secondary-500/25 ml-2 p-2"
						onclick={(event) => {
							event.stopPropagation();
							handleMaxIVs();
						}}
					>
						<BicepsFlexed />
					</button>
				</Tooltip>
			</div>
		{/snippet}
	</SectionHeader>
{/snippet}

{#snippet soulsHeader()}
	<SectionHeader text="Souls">
		{#snippet action()}
			<div class="flex">
				<Tooltip label="Max out Souls">
					<button
						class="btn hover:bg-secondary-500/25 ml-2 p-2"
						onclick={(event) => {
							event.stopPropagation();
							handleMaxSouls();
						}}
					>
						<BicepsFlexed />
					</button>
				</Tooltip>
			</div>
		{/snippet}
	</SectionHeader>
{/snippet}

{#if appState.selectedPal}
	<div class="flex h-full overflow-auto p-2">
		<div class="flex grow flex-col">
			<div class="w-2/3 shrink-0">
				<PalHeader bind:pal={appState.selectedPal} />
			</div>
			<div class="flex grow">
				<div class="hidden flex-1 overflow-auto p-2 2xl:block">
					<div class="flex flex-col space-y-2">
						{@render activeSkillsHeader()}
						{@render activeSkillsBody()}
						{@render passiveSkillsHeader()}
						{@render passiveSkillsBody()}

						<SectionHeader text="Presets" />
						<SkillPresets onSelect={setSkillPreset} />
						{@render workSuitabilityHeader()}
						<WorkSuitabilities bind:pal={appState.selectedPal} />
					</div>
				</div>
				<div class="mt-4 2xl:hidden">
					<Accordion
						classes="min-w-96 max-w-96"
						value={leftAccordionValue}
						onValueChange={(e: ValueChangeDetails) => (leftAccordionValue = e.value)}
						collapsible
					>
						<Accordion.Item value="active_skills" controlHover="hover:bg-secondary-500/25">
							{#snippet control()}
								{@render activeSkillsHeader()}
							{/snippet}
							{#snippet panel()}
								{@render activeSkillsBody()}
							{/snippet}
						</Accordion.Item>
						<Accordion.Item value="passive_skills" controlHover="hover:bg-secondary-500/25">
							{#snippet control()}
								{@render passiveSkillsHeader()}
							{/snippet}
							{#snippet panel()}
								{@render passiveSkillsBody()}
							{/snippet}
						</Accordion.Item>
						<Accordion.Item value="presets" controlHover="hover:bg-secondary-500/25">
							{#snippet control()}
								<SectionHeader text="Presets" />
							{/snippet}
							{#snippet panel()}
								<SkillPresets onSelect={setSkillPreset} />
							{/snippet}
						</Accordion.Item>
						<Accordion.Item value="work_suitability" controlHover="hover:bg-secondary-500/25">
							{#snippet control()}
								{@render workSuitabilityHeader()}
							{/snippet}
							{#snippet panel()}
								<WorkSuitabilities bind:pal={appState.selectedPal} />
							{/snippet}
						</Accordion.Item>
					</Accordion>
				</div>
				<div class="flex-1 overflow-auto p-2">
					<div class="flex h-full flex-col items-center justify-center">
						<div class="pal">
							<Tooltip
								popupClass="p-4 bg-surface-800"
								rounded="rounded-none"
								position="top-start"
								useArrow={false}
							>
								<div class="relative">
									<img
										src={palImage}
										alt={`${appState.selectedPal?.name} icon`}
										class="max-h-[350px] max-w-full 2xl:max-h-[600px]"
									/>
									{#if appState.selectedPal.is_predator}
										<img
											src={staticIcons.predatorIcon}
											alt="Predator"
											class="absolute bottom-0 right-0 h-12 w-12"
											style="filter: {calculateFilters('#FF0000')};"
										/>
									{/if}
								</div>

								{#snippet popup()}
									{#await getPalDescription(appState.selectedPal!.character_key) then description}
										{#if description}
											<div class="flex max-w-96 flex-col">
												<p class="text-center">{description}</p>
											</div>
										{/if}
									{/await}
								{/snippet}
							</Tooltip>
						</div>
					</div>
				</div>
			</div>
		</div>
		<div class="w-1/3 overflow-auto p-2">
			<div class="hidden flex-col space-y-2 2xl:flex">
				<StatusBadge bind:pal={appState.selectedPal} />
				<SectionHeader text="Stats" />
				<StatsBadges bind:pal={appState.selectedPal} bind:player={appState.selectedPlayer} />
				{@render talentsHeader()}
				<Talents bind:pal={appState.selectedPal} />
				{@render soulsHeader()}
				<Souls bind:pal={appState.selectedPal} />
			</div>
			<div class="flex flex-col space-y-2 2xl:hidden">
				<StatusBadge bind:pal={appState.selectedPal} />
				<Accordion
					classes="min-w-96"
					value={rightAccordionValue}
					onValueChange={(e: ValueChangeDetails) => (rightAccordionValue = e.value)}
					collapsible
				>
					<Accordion.Item value="stats" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							<SectionHeader text="Stats" />
						{/snippet}
						{#snippet panel()}
							<StatsBadges bind:pal={appState.selectedPal} bind:player={appState.selectedPlayer} />
						{/snippet}
					</Accordion.Item>
					<Accordion.Item value="talents" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							{@render talentsHeader()}
						{/snippet}
						{#snippet panel()}
							<Talents bind:pal={appState.selectedPal!} />
						{/snippet}
					</Accordion.Item>
					<Accordion.Item value="souls" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							{@render soulsHeader()}
						{/snippet}
						{#snippet panel()}
							<Souls bind:pal={appState.selectedPal!} />
						{/snippet}
					</Accordion.Item>
				</Accordion>
			</div>
		</div>
	</div>
{:else}
	<div class="flex w-full flex-col items-center justify-center">
		<h2 class="h2">Select a Pal to edit 🚀</h2>
		<p>
			Pals can be selected in a Players Pal Box, Guild Bases, Global Pal Storage, Dimensional Pal
			Storage, or Universal Pal Storage
		</p>
	</div>
{/if}
