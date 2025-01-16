<script lang="ts">
	import { PalHeader } from '$components';

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
	import { staticIcons } from '$lib/constants';
	import { palsData, expData, presetsData } from '$lib/data';
	import { getAppState, getModalState, getToastState } from '$states';
	import { BicepsFlexed, Brain, Save } from 'lucide-svelte';
	import { Souls } from '$components';
	import SkillPresets from './SkillPresets.svelte';
	import { assetLoader, calculateFilters } from '$utils';

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();

	let palLevelProgressToNext: number = $state(0);
	let palLevelProgressValue: number = $state(0);
	let palLevelProgressMax: number = $state(1);

	let palImage = $derived.by(() => {
		if (appState.selectedPal) {
			const { character_key } = appState.selectedPal;
			const palData = palsData.pals[character_key];
			return assetLoader.loadPalImage(character_key, palData?.is_pal);
		}
	});

	let activeSkills = $derived.by(() => {
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

	let passiveSkills = $derived.by(() => {
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
		const palData = palsData.pals[character_id];
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

	$effect(() => {
		calcPalLevelProgress();
	});

	function handleMaxIVs() {
		if (appState.selectedPal) {
			appState.selectedPal.talent_hp = 100;
			appState.selectedPal.talent_shot = 100;
			appState.selectedPal.talent_defense = 100;
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	function handleMaxSouls() {
		if (appState.selectedPal) {
			appState.selectedPal.rank_hp = 20;
			appState.selectedPal.rank_attack = 20;
			appState.selectedPal.rank_defense = 20;
			appState.selectedPal.rank_craftspeed = 20;
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	function handleMaxWorkSuitability() {
		if (!appState.selectedPal) return;
		const palData = palsData.pals[appState.selectedPal.character_key] ?? null;
		if (!palData) return;
		for (const [key, value] of Object.entries(palData.work_suitability)) {
			if (value === 0) continue;
			appState.selectedPal.work_suitability[key as WorkSuitability] = Math.min(5 - value, 4);
		}
		appState.selectedPal.state = EntryState.MODIFIED;
	}
</script>

{#if appState.selectedPal}
	<div class="flex h-full overflow-auto p-2">
		<div class="flex flex-grow flex-col">
			<div class="w-2/3 flex-shrink-0">
				<PalHeader bind:pal={appState.selectedPal} />
			</div>
			<div class="flex flex-grow">
				<div class="flex-1 overflow-auto p-2">
					<div class="flex flex-col space-y-2">
						<SectionHeader text="Active Skills">
							{#snippet action()}
								<div class="flex">
									<Tooltip>
										<button
											class="btn hover:bg-secondary-500/25 ml-2 p-2"
											onclick={handleEditLearnedSkills}
										>
											<Brain size={20} />
										</button>
										{#snippet popup()}
											<span>Edit Learned Skills</span>
										{/snippet}
									</Tooltip>
									<Tooltip>
										<button
											class="btn hover:bg-secondary-500/25 ml-2 p-2"
											onclick={() => handleAddPreset('active')}
										>
											<Save size={20} />
										</button>
										{#snippet popup()}
											<span>Save as preset</span>
										{/snippet}
									</Tooltip>
								</div>
							{/snippet}
						</SectionHeader>
						{#each activeSkills as skill}
							<ActiveSkillBadge
								{skill}
								onSkillUpdate={handleUpdateActiveSkill}
								palCharacterId={appState.selectedPal.character_key}
							/>
						{/each}
						<SectionHeader text="Passive Skills">
							{#snippet action()}
								<div class="flex">
									<Tooltip>
										<button
											class="btn hover:bg-secondary-500/25 ml-2 p-2"
											onclick={() => handleAddPreset('passive')}
										>
											<Save size={20} />
										</button>
										{#snippet popup()}
											<span>Save as preset</span>
										{/snippet}
									</Tooltip>
								</div>
							{/snippet}
						</SectionHeader>
						<div class="grid grid-cols-2 gap-2">
							{#each passiveSkills as skill}
								<PassiveSkillBadge {skill} onSkillUpdate={handleUpdatePassiveSkill} />
							{/each}
						</div>
						<SectionHeader text="Presets" />
						<SkillPresets onSelect={setSkillPreset} />
						<SectionHeader text="Work Suitability">
							{#snippet action()}
								<div class="flex">
									<Tooltip>
										<button
											class="btn hover:bg-secondary-500/25 ml-2 p-2"
											onclick={handleMaxWorkSuitability}
										>
											<BicepsFlexed />
										</button>
										{#snippet popup()}
											<span>Max out Work Suitability</span>
										{/snippet}
									</Tooltip>
								</div>
							{/snippet}
						</SectionHeader>
						<WorkSuitabilities bind:pal={appState.selectedPal} />
					</div>
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
										class="max-h-[600px] max-w-full"
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
			<div class="flex flex-col space-y-2">
				<StatsBadges bind:pal={appState.selectedPal} bind:player={appState.selectedPlayer} />
				<SectionHeader text="Talents (IVs)">
					{#snippet action()}
						<div class="flex">
							<Tooltip>
								<button class="btn hover:bg-secondary-500/25 ml-2 p-2" onclick={handleMaxIVs}>
									<BicepsFlexed />
								</button>
								{#snippet popup()}
									<span>Max out IVs</span>
								{/snippet}
							</Tooltip>
						</div>
					{/snippet}
				</SectionHeader>
				<Talents bind:pal={appState.selectedPal} />
				<SectionHeader text="Souls">
					{#snippet action()}
						<div class="flex">
							<Tooltip>
								<button class="btn hover:bg-secondary-500/25 ml-2 p-2" onclick={handleMaxSouls}>
									<BicepsFlexed />
								</button>
								{#snippet popup()}
									<span>Max out Souls</span>
								{/snippet}
							</Tooltip>
						</div>
					{/snippet}
				</SectionHeader>
				<Souls bind:pal={appState.selectedPal} />
			</div>
		</div>
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">Select a Player ➡️ Pal to edit 🚀</h2>
	</div>
{/if}
