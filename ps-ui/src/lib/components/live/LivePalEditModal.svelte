<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import {
		ActiveSkillBadge,
		PalHeader,
		PassiveSkillBadge,
		Souls,
		StatsBadges,
		StatusBadge,
		Talents,
		WorkSuitabilities
	} from '$components/pal';
	import {
		MultiSkillSelectModal,
		PalPresetSelectModal,
		PresetConfigModal,
		SkillPresetSelectModal,
		TextInputModal
	} from '$components/modals';
	import { Button, SectionHeader, Spinner, Tooltip } from '$components/ui';
	import { palsData, presetsData } from '$lib/data';
	import { getAppState, getModalState, getToastState } from '$states';
	import {
		defaultPresetConfig,
		type PalPresetConfig,
		type Player,
		type PresetProfile,
		type Pal,
		type WorkSuitability
	} from '$types';
	import { staticIcons } from '$types/icons';
	import { applyPalPreset, assetLoader, calculateFilters, handleMaxOutPal } from '$utils';
	import type {
		GamePalDetailJson,
		GamePalEditRequest,
		GamePalJson
	} from '$states/gameState.svelte';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/accordion';
	import * as m from '$i18n/messages';
	import { c, p } from '$lib/utils/commonTranslations';
	import { deriveCharacterKey, toLivePal } from './liveView.utils';
	import PalModelViewer from '$components/pal/PalModelViewer.svelte';

	let {
		pal: gamePal,
		detail,
		loading = false,
		levelCap,
		closeModal
	}: {
		pal: GamePalJson;
		detail?: GamePalDetailJson | null;
		loading?: boolean;
		levelCap?: number;
		closeModal: (value: GamePalEditRequest | undefined) => void;
	} = $props();

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();

	const palData = $derived(palsData.getByKey(deriveCharacterKey(gamePal.characterId)));

	const original = $derived(toLivePal(gamePal, palData, detail));

	const player = $derived({ level: levelCap ?? 0 } as Player);

	let draft = $state<Pal | undefined>(undefined);
	let seededFor = $state<string | null>(null);
	let leftAccordionValue = $state(['active_skills']);
	let rightAccordionValue = $state(['stats']);

	$effect(() => {
		const seed = original;
		if (seededFor === seed.instance_id) return;
		seededFor = seed.instance_id;
		draft = structuredClone($state.snapshot(seed)) as Pal;
	});

	const activeSkills = $derived.by(() => {
		const skills = [...(draft?.active_skills ?? [])];
		while (skills.length < 3) skills.push('Empty');
		return skills;
	});

	const passiveSkills = $derived.by(() => {
		const skills = [...(draft?.passive_skills ?? [])];
		while (skills.length < 4) skills.push('Empty');
		return skills;
	});

	const palImage = $derived(
		assetLoader.loadPalImage(original.character_key, palData?.is_pal || false)
	);

	function handleUpdateActiveSkill(newSkill: string, oldSkill: string) {
		if (!draft) return;
		const index = draft.active_skills.findIndex((skill) => skill === oldSkill);
		if (newSkill === 'Empty') {
			if (index >= 0) draft.active_skills.splice(index, 1);
		} else if (index >= 0) {
			draft.active_skills[index] = newSkill;
		} else {
			draft.active_skills.push(newSkill);
		}
	}

	function handleUpdatePassiveSkill(newSkill: string, oldSkill: string) {
		if (!draft) return;
		const index = draft.passive_skills.findIndex((skill) => skill === oldSkill);
		if (newSkill === 'Empty') {
			if (index >= 0) draft.passive_skills.splice(index, 1);
		} else if (index >= 0) {
			draft.passive_skills[index] = newSkill;
		} else {
			draft.passive_skills.push(newSkill);
		}
	}

	async function handleAddSkill(type: 'active' | 'passive') {
		if (!draft) return;
		// @ts-ignore
		const result = await modal.showModal<string[]>(MultiSkillSelectModal, {
			type: type === 'active' ? 'Active' : 'Passive',
			title: m.select_entity({ entity: type === 'active' ? c.activeSkill : c.passiveSkill }),
			pal: draft
		});
		if (!result) return;
		if (type === 'active') draft.active_skills.push(...result);
		else draft.passive_skills.push(...result);
	}

	async function handleApplySkillPreset(type: 'active' | 'passive') {
		if (!draft) return;
		// @ts-ignore
		const result = await modal.showModal<string[]>(SkillPresetSelectModal, {
			title: m.select_a_entity({
				entity: `${type === 'active' ? c.activeSkill : c.passiveSkill} ${c.preset}`
			}),
			type
		});
		if (!result) return;
		if (type === 'active') draft.active_skills = result;
		else draft.passive_skills = result;
	}

	async function handleAddSkillPreset(type: 'active' | 'passive') {
		if (!draft) return;
		// @ts-ignore
		const name = await modal.showModal<string>(TextInputModal, {
			title: m.add_skills_preset({ type }),
			value: '',
			inputLabel: m.preset_name()
		});
		if (!name) return;
		try {
			await presetsData.addPresetProfile({
				name,
				type: type === 'active' ? 'active_skills' : 'passive_skills',
				skills: type === 'active' ? draft.active_skills : draft.passive_skills
			} as PresetProfile);
		} catch (error) {
			console.error('Error adding preset:', error);
			toast.add(m.preset_save_failed(), m.error(), 'error');
		}
	}

	function handleMaxIVs() {
		if (!draft) return;
		const max = appState.settings.cheat_mode ? 255 : 100;
		draft.talent_hp = max;
		draft.talent_shot = max;
		draft.talent_defense = max;
	}

	function handleMaxSouls() {
		if (!draft) return;
		const max = appState.settings.cheat_mode ? 255 : 20;
		draft.rank_hp = max;
		draft.rank_attack = max;
		draft.rank_defense = max;
		draft.rank_craftspeed = max;
	}

	function handleMaxWorkSuitability() {
		if (!draft || !palData) return;
		for (const [key, value] of Object.entries(palData.work_suitability)) {
			if (value === 0) continue;
			draft.work_suitability[key as WorkSuitability] = Math.min(10 - value, 9);
		}
	}

	function keepSpecies(target: Pal) {
		target.character_id = original.character_id;
		target.character_key = original.character_key;
		target.is_boss = original.is_boss;
		target.is_lucky = original.is_lucky;
	}

	async function handleMaxOut() {
		if (!draft) return;
		await handleMaxOutPal(draft, player);
		keepSpecies(draft);
	}

	async function handleEditNickname() {
		if (!draft) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.edit_entity({ entity: m.nickname() }),
			value: draft.nickname || draft.name
		});
		if (result === undefined) return;
		draft.nickname = result;
	}

	async function handleApplyPalPreset() {
		if (!draft) return;
		// @ts-ignore
		const result = await modal.showModal<string>(PalPresetSelectModal, {
			title: m.select_entity({ entity: c.preset }),
			selectedPals: [{ character_id: draft.character_id, character_key: draft.character_key }]
		});
		if (!result) return;
		applyPalPreset(draft, presetsData.presetProfiles[result], player);
		keepSpecies(draft);
	}

	async function handleSavePalPreset() {
		if (!draft) return;
		const element = palData?.element_types[0];
		// @ts-ignore
		const result = await modal.showModal(PresetConfigModal, {
			config: defaultPresetConfig,
			palName: draft.name,
			element
		});
		if (!result) return;
		const { name, config } = result as { name: string; config: PalPresetConfig };
		try {
			await presetsData.addPresetProfile({
				name,
				type: 'pal_preset',
				pal_preset: {
					lock: config.lock,
					character_id: draft.character_id,
					is_lucky: config.is_lucky ? draft.is_lucky : null,
					is_boss: config.is_boss ? draft.is_boss : null,
					is_awakened: config.is_awakened ? draft.is_awakened : null,
					gender: config.gender ? draft.gender : null,
					rank_hp: config.rank_hp ? draft.rank_hp : null,
					rank_attack: config.rank_attack ? draft.rank_attack : null,
					rank_defense: config.rank_defense ? draft.rank_defense : null,
					rank_craftspeed: config.rank_craftspeed ? draft.rank_craftspeed : null,
					talent_hp: config.talent_hp ? draft.talent_hp : null,
					talent_shot: config.talent_shot ? draft.talent_shot : null,
					talent_defense: config.talent_defense ? draft.talent_defense : null,
					rank: config.rank ? draft.rank : null,
					level: config.level ? draft.level : null,
					active_skills: config.active_skills ? draft.active_skills : null,
					passive_skills: config.passive_skills ? draft.passive_skills : null,
					work_suitability: config.work_suitability ? draft.work_suitability : null,
					sanity: config.sanity ? draft.sanity : null,
					exp: config.exp ? draft.exp : null,
					element,
					lock_element: config.lock_element,
					nickname: config.nickname ? draft.nickname : null,
					stomach: config.stomach ? draft.stomach : null,
					hp: config.hp ? draft.hp : null,
					friendship_point: config.friendship_point ? draft.friendship_point : null
				}
			} as PresetProfile);
		} catch (error) {
			console.error('Error adding preset:', error);
			toast.add(m.preset_save_failed(), m.error(), 'error');
		}
	}

	const sameList = (a: string[], b: string[]) =>
		a.length === b.length && [...a].sort().join(',') === [...b].sort().join(',');

	function changes(): GamePalEditRequest {
		const edited: GamePalEditRequest = {};
		if (!draft) return edited;
		const base = original;

		if (draft.level !== base.level) edited.level = draft.level;
		if (draft.exp !== base.exp) edited.exp = draft.exp;
		if (draft.rank !== base.rank) edited.rank = draft.rank;
		if ((draft.nickname ?? '') !== (base.nickname ?? '')) edited.nickname = draft.nickname ?? '';
		if (draft.talent_hp !== base.talent_hp) edited.talentHp = draft.talent_hp;
		if (draft.talent_shot !== base.talent_shot) edited.talentShot = draft.talent_shot;
		if (draft.talent_defense !== base.talent_defense) edited.talentDefense = draft.talent_defense;
		if (draft.rank_hp !== base.rank_hp) edited.rankHp = draft.rank_hp;
		if (draft.rank_attack !== base.rank_attack) edited.rankAttack = draft.rank_attack;
		if (draft.rank_defense !== base.rank_defense) edited.rankDefense = draft.rank_defense;
		if (draft.rank_craftspeed !== base.rank_craftspeed)
			edited.rankCraftSpeed = draft.rank_craftspeed;
		if (draft.friendship_point !== base.friendship_point)
			edited.friendshipPoint = draft.friendship_point;
		if (draft.sanity !== base.sanity) edited.sanity = draft.sanity;
		if (draft.stomach !== base.stomach) edited.stomach = draft.stomach;
		if (draft.hp !== base.hp) edited.hp = draft.hp;
		if (draft.gender !== base.gender) edited.gender = draft.gender.toLowerCase();
		if (draft.is_awakened !== base.is_awakened) edited.isAwakened = draft.is_awakened;

		if (!sameList(draft.active_skills ?? [], base.active_skills ?? []))
			edited.activeSkills = draft.active_skills;
		if (!sameList(draft.passive_skills ?? [], base.passive_skills ?? []))
			edited.passiveSkills = draft.passive_skills;

		const suitability = (draft.work_suitability ?? {}) as Record<string, number>;
		if (JSON.stringify(suitability) !== JSON.stringify(base.work_suitability ?? {}))
			edited.workSuitability = suitability;

		return edited;
	}
</script>

{#snippet activeSkillsHeader()}
	<SectionHeader text={c.activeSkills}>
		{#snippet action()}
			<div class="flex">
				<Tooltip label={m.save_as_preset()}>
					<Button
						variant="ghost"
						size="icon"
						class="ml-2"
						onclick={(event: MouseEvent) => {
							event.stopPropagation();
							handleAddSkillPreset('active');
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
			<ActiveSkillBadge {skill} pal={draft} onSkillUpdate={handleUpdateActiveSkill} />
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
							handleAddSkillPreset('passive');
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
			<PassiveSkillBadge {skill} pal={draft} onSkillUpdate={handleUpdatePassiveSkill} />
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

{#snippet palImageFallback()}
	<img src={palImage} alt={`${appState.selectedPal?.name} icon`} class="size-full object-contain" />
{/snippet}

<div class="bg-surface-950 h-full w-full overflow-hidden md:h-[90vh] md:w-[90vw] md:rounded-sm">
	{#if loading || !draft}
		<div class="flex h-full items-center justify-center">
			<Spinner size="size-16" />
		</div>
	{:else}
		<div id="live-pal-edit" class="flex h-full flex-col overflow-auto p-2 md:flex-row">
			<nav
				id="live-pal-quick-actions"
				class="btn-group preset-outlined-surface-200-800 mb-2 flex justify-center rounded-sm md:mr-2 md:mb-0 md:flex-col md:items-center md:self-start"
				aria-label={m.live_edit_pal({ name: original.name })}
			>
				<Tooltip label={m.edit_entity({ entity: m.nickname() })}>
					<Button variant="ghost" size="icon" onclick={handleEditNickname}>
						<Icon icon="tabler:edit" class="h-6 w-6" />
					</Button>
				</Tooltip>
				<Tooltip label={m.max_out_pal_stats(p.pal)}>
					<Button variant="ghost" size="icon" onclick={handleMaxOut}>
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
				<Tooltip label={m.save_changes()}>
					<Button
						variant="ghost"
						size="icon"
						class="border-surface-700 border-t"
						onclick={() => closeModal(changes())}
						data-modal-primary
					>
						<Icon icon="tabler:check" class="text-primary-500 h-6 w-6" />
					</Button>
				</Tooltip>
				<Tooltip label={m.cancel()}>
					<Button variant="ghost" size="icon" onclick={() => closeModal(undefined)}>
						<Icon icon="tabler:x" class="h-6 w-6" />
					</Button>
				</Tooltip>
			</nav>

			<div class="flex grow flex-col">
				<div id="live-pal-header" class="w-full shrink-0 md:w-3/4 2xl:w-2/3">
					<PalHeader bind:pal={draft} showSpeciesActions={false} {levelCap} />
				</div>
				<div class="flex grow flex-col md:flex-row">
					<div class="hidden flex-1 overflow-auto p-2 2xl:block">
						<div class="flex flex-col space-y-2">
							<div id="live-pal-active-skills">
								{@render activeSkillsHeader()}
								{@render activeSkillsBody()}
							</div>
							<div id="live-pal-passive-skills">
								{@render passiveSkillsHeader()}
								{@render passiveSkillsBody()}
							</div>
							<div id="live-pal-work-suitability">
								{@render workSuitabilityHeader()}
								<WorkSuitabilities bind:pal={draft} />
							</div>
						</div>
					</div>
					<div class="mt-4 2xl:hidden">
						<Accordion
							classes="w-full md:min-w-96 md:max-w-96"
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
									<WorkSuitabilities bind:pal={draft} />
								{/snippet}
							</Accordion.Item>
						</Accordion>
					</div>
					<div id="live-pal-image" class="flex-1 overflow-auto p-2">
						<div class="flex h-full flex-col items-center justify-center">
							<Tooltip
								baseClass="w-full"
								popupClass="p-4 bg-surface-800"
								rounded="rounded-none"
								position="top-start"
								useArrow={false}
							>
								<div class="relative h-48 w-full md:h-87.5 2xl:h-150">
									<PalModelViewer characterKey={draft.character_key} fallback={palImageFallback} />
									{#if draft.is_predator}
										<img
											src={staticIcons.predatorIcon}
											alt="Predator"
											class="absolute right-0 bottom-0 h-12 w-12"
											style="filter: {calculateFilters('#FF0000')};"
										/>
									{/if}
								</div>
								{#snippet popup()}
									{#if palData?.description}
										<div class="flex max-w-96 flex-col">
											<p class="text-center">{palData.description}</p>
										</div>
									{/if}
								{/snippet}
							</Tooltip>
						</div>
					</div>
				</div>
			</div>

			<div class="w-full overflow-auto p-2 md:w-1/3">
				<div class="hidden flex-col space-y-2 2xl:flex">
					<div id="live-pal-status">
						<StatusBadge bind:pal={draft} />
					</div>
					<div id="live-pal-stats">
						<SectionHeader text={m.stats()} />
						<StatsBadges bind:pal={draft} {player} />
					</div>
					<div id="live-pal-talents">
						{@render talentsHeader()}
						<Talents bind:pal={draft} />
					</div>
					<div id="live-pal-souls">
						{@render soulsHeader()}
						<Souls bind:pal={draft} />
					</div>
				</div>
				<div class="flex flex-col space-y-2 2xl:hidden">
					<StatusBadge bind:pal={draft} />
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
								<StatsBadges bind:pal={draft} {player} />
							{/snippet}
						</Accordion.Item>
						<Accordion.Item value="talents" controlHover="hover:bg-secondary-500/25">
							{#snippet control()}
								{@render talentsHeader()}
							{/snippet}
							{#snippet panel()}
								<Talents bind:pal={draft!} />
							{/snippet}
						</Accordion.Item>
						<Accordion.Item value="souls" controlHover="hover:bg-secondary-500/25">
							{#snippet control()}
								{@render soulsHeader()}
							{/snippet}
							{#snippet panel()}
								<Souls bind:pal={draft!} />
							{/snippet}
						</Accordion.Item>
					</Accordion>
				</div>
			</div>
		</div>
	{/if}
</div>
