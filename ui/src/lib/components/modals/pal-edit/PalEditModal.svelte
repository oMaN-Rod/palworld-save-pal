<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import {
		ActiveSkillBadge,
		PalHeader,
		PalModelViewer,
		PassiveSkillBadge,
		Souls,
		StatsBadges,
		StatusBadge,
		Talents,
		WorkSuitabilities
	} from '$components/pal';
	import {
		LearnedSkillSelectModal,
		MultiSkillSelectModal,
		PalPresetSelectModal,
		PresetConfigModal,
		SkillPresetSelectModal,
		TextInputModal
	} from '$components/modals';
	import { Button, SectionHeader, Tooltip } from '$components/ui';
	import { expData, palsData, presetsData } from '$lib/data';
	import { getAppState, getModalState } from '$states';
	import {
		defaultPresetConfig,
		EntryState,
		type PalPresetConfig,
		type PresetProfile,
		type WorkSuitability
	} from '$types';
	import { staticIcons } from '$types/icons';
	import { assetLoader, calculateFilters, formatBossCharacterId, handleMaxOutPal } from '$utils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/accordion';
	import * as m from '$i18n/messages';
	import { c, p } from '$lib/utils/commonTranslations';

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

	async function handleApplySkillPreset(type: 'active' | 'passive') {
		if (!appState.selectedPal) return;
		// @ts-ignore
		const result = await modal.showModal<string[]>(SkillPresetSelectModal, {
			title: m.select_a_entity({
				entity: `${type === 'active' ? c.activeSkill : c.passiveSkill} ${c.preset}`
			}),
			type
		});
		if (!result) return;
		await setSkillPreset(type, result);
	}

	async function handleAddPreset(type: 'active' | 'passive') {
		if (!appState.selectedPal) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.add_skills_preset({ type }),
			value: '',
			inputLabel: m.preset_name()
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
			appState.selectedPal.work_suitability[key as WorkSuitability] = Math.min(10 - value, 9);
		}
		appState.selectedPal.state = EntryState.MODIFIED;
	}

	async function handleAddSkill(type: 'active' | 'passive') {
		// @ts-ignore
		const result = await modal.showModal<string[]>(MultiSkillSelectModal, {
			type: type === 'active' ? 'Active' : 'Passive',
			title: m.select_entity({ entity: type === 'active' ? c.activeSkill : c.passiveSkill }),
			pal: appState.selectedPal
		});
		if (!result) return;
		if (type === 'active') {
			appState.selectedPal!.active_skills.push(...result);
		} else {
			appState.selectedPal!.passive_skills.push(...result);
		}
	}

	async function handleEditNickname() {
		const pal = appState.selectedPal;
		if (!pal) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.edit_entity({ entity: m.nickname() }),
			value: pal.nickname || pal.name
		});
		if (!result) return;
		pal.nickname = result;
		pal.state = EntryState.MODIFIED;
		if (appState.selectedPlayer && appState.selectedPlayer.pals)
			appState.selectedPlayer.pals[pal.instance_id].nickname = result;
	}

	async function handleApplyPalPreset() {
		const pal = appState.selectedPal;
		if (!pal) return;
		// @ts-ignore
		const result = await modal.showModal<string>(PalPresetSelectModal, {
			title: m.select_entity({ entity: c.preset }),
			selectedPals: [{ character_id: pal.character_id, character_key: pal.character_key }]
		});
		if (!result) return;

		const presetProfile = presetsData.presetProfiles[result];

		for (const [key, value] of Object.entries(presetProfile.pal_preset!)) {
			if (key === 'character_id') continue;
			if (key === 'lock' && value) {
				pal.character_id = presetProfile.pal_preset?.character_id as string;
			}
			if (key === 'is_boss' && value && pal.is_lucky) {
				pal.is_boss = true;
				pal.is_lucky = false;
			}
			if (key === 'is_lucky' && value && pal.is_boss) {
				pal.is_boss = false;
				pal.is_lucky = true;
			} else if (value !== null) {
				(pal as Record<string, any>)[key] = value;
			}
		}
		formatBossCharacterId(pal);
		pal.state = EntryState.MODIFIED;
	}

	async function handleSavePalPreset() {
		const pal = appState.selectedPal;
		if (!pal) return;
		const element = palsData.getByKey(pal.character_key)?.element_types[0];
		// @ts-ignore
		const result = await modal.showModal(PresetConfigModal, {
			config: defaultPresetConfig,
			palName: pal.name,
			element
		});
		if (!result) return;

		const { name, config } = result as { name: string; config: PalPresetConfig };

		const newPreset = {
			name: name,
			type: 'pal_preset',
			pal_preset: {
				lock: config.lock,
				character_id: pal.character_id,
				is_lucky: config.is_lucky ? pal.is_lucky : null,
				is_boss: config.is_boss ? pal.is_boss : null,
				is_awakened: config.is_awakened ? pal.is_awakened : null,
				is_imported: config.is_imported ? pal.is_imported : null,
				gender: config.gender ? pal.gender : null,
				rank_hp: config.rank_hp ? pal.rank_hp : null,
				rank_attack: config.rank_attack ? pal.rank_attack : null,
				rank_defense: config.rank_defense ? pal.rank_defense : null,
				rank_craftspeed: config.rank_craftspeed ? pal.rank_craftspeed : null,
				talent_hp: config.talent_hp ? pal.talent_hp : null,
				talent_shot: config.talent_shot ? pal.talent_shot : null,
				talent_defense: config.talent_defense ? pal.talent_defense : null,
				rank: config.rank ? pal.rank : null,
				level: config.level ? pal.level : null,
				learned_skills: config.learned_skills ? pal.learned_skills : null,
				active_skills: config.active_skills ? pal.active_skills : null,
				passive_skills: config.passive_skills ? pal.passive_skills : null,
				work_suitability: config.work_suitability ? pal.work_suitability : null,
				sanity: config.sanity ? pal.sanity : null,
				exp: config.exp ? pal.exp : null,
				element: element,
				lock_element: config.lock_element,
				nickname: config.nickname ? pal.nickname : null,
				filtered_nickname: config.filtered_nickname ? pal.nickname : null,
				stomach: config.stomach ? pal.stomach : null,
				hp: config.hp ? pal.hp : null,
				friendship_point: config.friendship_point ? pal.friendship_point : null
			}
		} as PresetProfile;

		await presetsData.addPresetProfile(newPreset);
	}

	$effect(() => {
		calcPalLevelProgress();
	});
</script>

{#snippet palImageFallback()}
	<img src={palImage} alt={`${appState.selectedPal?.name} icon`} class="size-full object-contain" />
{/snippet}

{#snippet activeSkillsHeader()}
	<SectionHeader text={c.activeSkills}>
		{#snippet action()}
			<div class="flex">
				<Tooltip label={m.learned_skills()}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleEditLearnedSkills();
						}}
					>
						<Icon icon="tabler:brain" size={20} />
					</Button>
				</Tooltip>
				<Tooltip label={m.save_as_preset()}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleAddPreset('active');
						}}
					>
						<Icon icon="tabler:device-floppy" size={20} />
					</Button>
				</Tooltip>
				<Tooltip label={m.apply_preset()}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleApplySkillPreset('active');
						}}
					>
						<Icon icon="tabler:player-play" size={20} />
					</Button>
				</Tooltip>
				<Tooltip label={m.add_entity({ entity: c.activeSkill })}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleAddSkill('active');
						}}
					>
						<Icon icon="tabler:plus" size={20} />
					</Button>
				</Tooltip>
			</div>
		{/snippet}
	</SectionHeader>
{/snippet}

{#snippet activeSkillsBody()}
	<div class="flex max-h-36 flex-col space-y-2 overflow-y-auto">
		{#each activeSkills as skill}
			<ActiveSkillBadge
				{skill}
				pal={appState.selectedPal}
				onSkillUpdate={handleUpdateActiveSkill}
			/>
		{/each}
	</div>
{/snippet}

{#snippet passiveSkillsHeader()}
	<SectionHeader text={c.passiveSkills}>
		{#snippet action()}
			<div class="flex">
				<Tooltip label={m.save_as_preset()}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleAddPreset('passive');
						}}
					>
						<Icon icon="tabler:device-floppy" size={20} />
					</Button>
				</Tooltip>
				<Tooltip label={m.apply_preset()}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleApplySkillPreset('passive');
						}}
					>
						<Icon icon="tabler:player-play" size={20} />
					</Button>
				</Tooltip>
				<Tooltip label={m.add_entity({ entity: c.passiveSkill })}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleAddSkill('passive');
						}}
					>
						<Icon icon="tabler:plus" size={20} />
					</Button>
				</Tooltip>
			</div>
		{/snippet}
	</SectionHeader>
{/snippet}

{#snippet passiveSkillsBody()}
	<div class="grid max-h-24 grid-cols-2 gap-2 overflow-y-auto">
		{#each passiveSkills as skill}
			<PassiveSkillBadge
				{skill}
				pal={appState.selectedPal}
				onSkillUpdate={handleUpdatePassiveSkill}
			/>
		{/each}
	</div>
{/snippet}

{#snippet workSuitabilityHeader()}
	<SectionHeader text={m.work_suitability()}>
		{#snippet action()}
			<div class="flex">
				<Tooltip label={m.max_work_suitability()}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleMaxWorkSuitability();
						}}
					>
						<Icon icon="ph:hand-fist" />
					</Button>
				</Tooltip>
			</div>
		{/snippet}
	</SectionHeader>
{/snippet}

{#snippet talentsHeader()}
	<SectionHeader text={m.talents_ivs()}>
		{#snippet action()}
			<div class="flex">
				<Tooltip label={m.max_ivs()}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleMaxIVs();
						}}
					>
						<Icon icon="ph:hand-fist" />
					</Button>
				</Tooltip>
			</div>
		{/snippet}
	</SectionHeader>
{/snippet}

{#snippet soulsHeader()}
	<SectionHeader text={m.souls()}>
		{#snippet action()}
			<div class="flex">
				<Tooltip label={m.max_souls()}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleMaxSouls();
						}}
					>
						<Icon icon="ph:hand-fist" />
					</Button>
				</Tooltip>
			</div>
		{/snippet}
	</SectionHeader>
{/snippet}

{#if appState.selectedPal}
	<div class="flex h-full overflow-auto p-2">
		<nav
			id="pal-quick-actions"
			class="btn-group preset-outlined-surface-200-800 mr-2 flex-col items-center self-start rounded-sm"
		>
			<Tooltip label={m.edit_entity({ entity: m.nickname() })}>
				<Button variant="ghost" size="icon" onclick={handleEditNickname}>
					<Icon icon="tabler:edit" class="h-6 w-6" />
				</Button>
			</Tooltip>
			<Tooltip label={m.max_out_pal_stats(p.pal)}>
				<Button
					variant="ghost"
					size="icon"
					onclick={() => handleMaxOutPal(appState.selectedPal!, appState.selectedPlayer!)}
				>
					<Icon icon="ph:hand-fist" class="h-6 w-6" />
				</Button>
			</Tooltip>
			<Tooltip label={m.save_as_preset()}>
				<Button variant="ghost" size="icon" onclick={handleSavePalPreset}>
					<Icon icon="tabler:device-floppy" class="h-6 w-6" />
				</Button>
			</Tooltip>
			<Tooltip label={m.apply_preset()}>
				<Button variant="ghost" size="icon" onclick={handleApplyPalPreset}>
					<Icon icon="tabler:player-play" class="h-6 w-6" />
				</Button>
			</Tooltip>
		</nav>
		<div class="flex grow flex-col">
			<div id="pal-header" class="w-3/4 shrink-0 2xl:w-2/3">
				<PalHeader bind:pal={appState.selectedPal} />
			</div>
			<div class="flex grow">
				<div class="hidden flex-1 overflow-auto p-2 2xl:block">
					<div class="flex flex-col space-y-2">
						<div id="pal-active-skills">
							{@render activeSkillsHeader()}
							{@render activeSkillsBody()}
						</div>
						<div id="pal-passive-skills">
							{@render passiveSkillsHeader()}
							{@render passiveSkillsBody()}
						</div>
						<div id="pal-work-suitability">
							{@render workSuitabilityHeader()}
							<WorkSuitabilities bind:pal={appState.selectedPal} />
						</div>
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
				<div id="pal-image" class="flex-1 overflow-auto p-2">
					<div class="flex h-full flex-col items-center justify-center">
						<div class="pal w-full">
							<Tooltip
								baseClass="w-full"
								popupClass="p-4 bg-surface-800"
								rounded="rounded-none"
								position="top-start"
								useArrow={false}
							>
								<div class="relative h-87.5 w-full 2xl:h-150">
									<PalModelViewer
										characterKey={appState.selectedPal.character_key}
										fallback={palImageFallback}
									/>
									{#if appState.selectedPal.is_predator}
										<img
											src={staticIcons.predatorIcon}
											alt="Predator"
											class="absolute right-0 bottom-0 h-12 w-12"
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
				<div id="pal-status">
					<StatusBadge bind:pal={appState.selectedPal} />
				</div>
				<div id="pal-stats">
					<SectionHeader text={m.stats()} />
					<StatsBadges bind:pal={appState.selectedPal} bind:player={appState.selectedPlayer} />
				</div>
				<div id="pal-talents">
					{@render talentsHeader()}
					<Talents bind:pal={appState.selectedPal} />
				</div>
				<div id="pal-souls">
					{@render soulsHeader()}
					<Souls bind:pal={appState.selectedPal} />
				</div>
			</div>
			<div class="flex flex-col space-y-2 2xl:hidden">
				<StatusBadge bind:pal={appState.selectedPal} />
				<Accordion
					classes="w-full min-w-0 2xl:min-w-96"
					value={rightAccordionValue}
					onValueChange={(e: ValueChangeDetails) => (rightAccordionValue = e.value)}
					collapsible
				>
					<Accordion.Item value="stats" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							<SectionHeader text={m.stats()} />
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
{/if}
