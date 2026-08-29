<script lang="ts">
	import { SectionHeader } from '$components/ui';
	import { EntryState, type Pal } from '$types';
	import ActiveSkillBadge from './ActiveSkillBadge.svelte';
	import StatusBadge from './StatusBadge.svelte';
	import PalHeader from './PalHeader.svelte';
	import PassiveSkillBadge from './PassiveSkillBadge.svelte';
	import { palsData } from '$lib/data';
	import { staticIcons } from '$types/icons';
	import { NumberSliderModal } from '$components/modals';
	import { getAppState, getModalState } from '$states';
	import { onMount } from 'svelte';
	import { Tween } from 'svelte/motion';
	import { cubicOut } from 'svelte/easing';
	import * as m from '$i18n/messages';
	import { c } from '$utils/commonTranslations';

	let { pal = $bindable() }: {
		pal: Pal;
	} = $props();

	const appState = getAppState();
	const modal = getModalState();

	type TalentKey = 'talent_hp' | 'talent_shot' | 'talent_defense';
	const talentLabels: Record<TalentKey, string> = {
		talent_hp: m.hp(),
		talent_shot: m.attack(),
		talent_defense: m.defense()
	};
	type SoulKey = 'rank_hp' | 'rank_attack' | 'rank_defense' | 'rank_craftspeed';
	const soulLabels: Record<SoulKey, string> = {
		rank_hp: m.hp(),
		rank_attack: m.attack(),
		rank_defense: m.defense(),
		rank_craftspeed: m.work_speed()
	};

	const maxTalent = $derived(appState.settings.cheat_mode ? 255 : 100);
	const maxSouls = $derived(appState.settings.cheat_mode ? 255 : 20);
	const talentMarkers = $derived(appState.settings.cheat_mode ? [50, 100, 150, 200] : [25, 50, 75]);
	const soulMarkers = $derived(appState.settings.cheat_mode ? [50, 100, 150, 200] : [5, 10, 15]);

	async function handleEditTalent(stat: TalentKey): Promise<void> {
		console.log('handleEditTalent', stat, pal[stat]);
		// @ts-ignore
		const result = await modal.showModal<number>(NumberSliderModal, {
			title: m.edit_entity({ entity: `${talentLabels[stat]} ${m.iv()}` }),
			value: pal[stat],
			min: 0,
			max: maxTalent,
			markers: talentMarkers
		});
		if (result === null || result === undefined) return;
		pal[stat] = result;
		pal.state = EntryState.MODIFIED;
	}

	async function handleEditSoul(stat: SoulKey): Promise<void> {
		// @ts-ignore
		const result = await modal.showModal<number>(NumberSliderModal, {
			title: m.edit_entity({ entity: `${soulLabels[stat]} ${m.souls()}` }),
			value: pal[stat],
			min: 0,
			max: maxSouls,
			markers: soulMarkers
		});
		if (result === null || result === undefined) return;
		pal[stat] = result;
		pal.state = EntryState.MODIFIED;
	}

	let originalActiveSkills = $derived(pal ? [...pal.active_skills] : []);
	let originalPassiveSkills = $derived(pal ? [...pal.passive_skills] : []);

	let activeSkillIndex = $state(0);
	let passiveSkillIndex = $state(0);

	let activeSkillsToShow = $state<string[]>(getInitialActiveSkills());
	let passiveSkillsToShow = $state<string[]>(getInitialPassiveSkills());

	const ROTATION_INTERVAL = 2000;
	let activeSkillsIntervalId: number;
	let passiveSkillsIntervalId: number;

	const activeProgress = new Tween(1, { duration: 500, easing: cubicOut });
	const passiveProgress = new Tween(1, { duration: 500, easing: cubicOut });

	const palData = $derived(palsData.getByKey(pal.character_key));

	function getInitialActiveSkills() {
		if (!pal || !pal.active_skills || pal.active_skills.length === 0) {
			return ['Empty', 'Empty', 'Empty'];
		}

		if (pal.active_skills.length <= 3) {
			const skills = [...pal.active_skills];
			while (skills.length < 3) {
				skills.push('Empty');
			}
			return skills;
		} else {
			return pal.active_skills.slice(0, 3);
		}
	}

	function getInitialPassiveSkills() {
		if (!pal || !pal.passive_skills || pal.passive_skills.length === 0) {
			return ['Empty', 'Empty', 'Empty', 'Empty'];
		}

		if (pal.passive_skills.length <= 4) {
			const skills = [...pal.passive_skills];
			while (skills.length < 4) {
				skills.push('Empty');
			}
			return skills;
		} else {
			return pal.passive_skills.slice(0, 4);
		}
	}

	function updateActiveSkills() {
		if (originalActiveSkills.length > 3) {
			activeProgress.set(0).then(() => {
				activeSkillsToShow = [];
				for (let i = 0; i < 3; i++) {
					const index = (activeSkillIndex + i) % originalActiveSkills.length;
					activeSkillsToShow.push(originalActiveSkills[index]);
				}
				activeProgress.set(1);
			});

			activeSkillIndex = (activeSkillIndex + 1) % originalActiveSkills.length;
		} else {
			activeSkillsToShow = [...originalActiveSkills];
			while (activeSkillsToShow.length < 3) {
				activeSkillsToShow.push('Empty');
			}
		}
	}

	function updatePassiveSkills() {
		if (originalPassiveSkills.length > 4) {
			passiveProgress.set(0).then(() => {
				passiveSkillsToShow = [];
				for (let i = 0; i < 4; i++) {
					const index = (passiveSkillIndex + i) % originalPassiveSkills.length;
					passiveSkillsToShow.push(originalPassiveSkills[index]);
				}
				passiveProgress.set(1);
			});

			passiveSkillIndex = (passiveSkillIndex + 1) % originalPassiveSkills.length;
		} else {
			passiveSkillsToShow = [...originalPassiveSkills];
			while (passiveSkillsToShow.length < 4) {
				passiveSkillsToShow.push('Empty');
			}
		}
	}

	$effect(() => {
		activeSkillIndex = 0;
		passiveSkillIndex = 0;

		activeSkillsToShow = getInitialActiveSkills();
		passiveSkillsToShow = getInitialPassiveSkills();
	});

	onMount(() => {
		if (originalActiveSkills.length > 3) {
			activeSkillsIntervalId = window.setInterval(updateActiveSkills, ROTATION_INTERVAL);
		}

		if (originalPassiveSkills.length > 4) {
			passiveSkillsIntervalId = window.setInterval(updatePassiveSkills, ROTATION_INTERVAL);
		}

		return () => {
			if (activeSkillsIntervalId) window.clearInterval(activeSkillsIntervalId);
			if (passiveSkillsIntervalId) window.clearInterval(passiveSkillsIntervalId);
		};
	});
	``;

	function handleUpdateActiveSkill(newSkill: string, oldSkill: string): void {
		const targetSkillIndex = pal.active_skills.findIndex((s) => s === oldSkill);

		if (newSkill === 'Empty') {
			if (targetSkillIndex >= 0) {
				pal.active_skills.splice(targetSkillIndex, 1);
			}
		} else {
			if (targetSkillIndex >= 0) {
				pal.active_skills[targetSkillIndex] = newSkill;
			} else {
				pal.active_skills.push(newSkill);
			}
		}

		pal.state = EntryState.MODIFIED;
	}

	function handleUpdatePassiveSkill(newSkill: string, oldSkill: string): void {
		const targetSkillIndex = pal.passive_skills.findIndex((s) => s === oldSkill);

		if (newSkill === 'Empty') {
			if (targetSkillIndex >= 0) {
				pal.passive_skills.splice(targetSkillIndex, 1);
			}
		} else {
			if (targetSkillIndex >= 0) {
				pal.passive_skills[targetSkillIndex] = newSkill;
			} else {
				pal.passive_skills.push(newSkill);
			}
		}

		pal.state = EntryState.MODIFIED;
	}
</script>

<div class="flex w-112.5 flex-col space-y-2">
	<PalHeader bind:pal showActions={false} popup />
	<StatusBadge bind:pal />
	<div class="flex justify-center space-x-2">
		<span class="text-surface-300 mr-1 text-xs">{m.ivs()}</span>
		<button
			type="button"
			class="chip hover:ring-secondary-500 bg-green-700 hover:ring"
			onclick={() => handleEditTalent('talent_hp')}
			aria-label={m.hp()}
		>
			<img src={staticIcons.hpIcon} alt={m.hp()} class="h-4 w-4" />
			<span class="text-sm font-bold">{pal.talent_hp}</span>
		</button>
		<button
			type="button"
			class="chip hover:ring-secondary-500 bg-red-700 hover:ring"
			onclick={() => handleEditTalent('talent_shot')}
			aria-label={m.attack()}
		>
			<img src={staticIcons.attackIcon} alt={m.attack()} class="h-6 w-6" />
			<span class="text-sm font-bold">{pal.talent_shot}</span>
		</button>
		<button
			type="button"
			class="chip hover:ring-secondary-500 bg-blue-700 hover:ring"
			onclick={() => handleEditTalent('talent_defense')}
			aria-label={m.defense()}
		>
			<img src={staticIcons.defenseIcon} alt={m.defense()} class="h-6 w-6" />
			<span class="text-sm font-bold">{pal.talent_defense}</span>
		</button>
	</div>
	<div class="flex justify-center space-x-2">
		<span class="text-surface-300 mr-1 text-xs">{m.souls()}</span>
		<button
			type="button"
			class="chip hover:ring-secondary-500 bg-green-700 hover:ring"
			onclick={() => handleEditSoul('rank_hp')}
			aria-label={m.hp()}
		>
			<img src={staticIcons.hpIcon} alt={m.hp()} class="h-4 w-4" />
			<span class="text-sm font-bold">{pal.rank_hp}</span>
		</button>
		<button
			type="button"
			class="chip hover:ring-secondary-500 bg-red-700 hover:ring"
			onclick={() => handleEditSoul('rank_attack')}
			aria-label={m.attack()}
		>
			<img src={staticIcons.attackIcon} alt={m.attack()} class="h-6 w-6" />
			<span class="text-sm font-bold">{pal.rank_attack}</span>
		</button>
		<button
			type="button"
			class="chip hover:ring-secondary-500 bg-blue-700 hover:ring"
			onclick={() => handleEditSoul('rank_defense')}
			aria-label={m.defense()}
		>
			<img src={staticIcons.defenseIcon} alt={m.defense()} class="h-6 w-6" />
			<span class="text-sm font-bold">{pal.rank_defense}</span>
		</button>
		<button
			type="button"
			class="chip hover:ring-secondary-500 bg-purple-700 hover:ring"
			onclick={() => handleEditSoul('rank_craftspeed')}
			aria-label={m.work_speed()}
		>
			<img src={staticIcons.workSpeedIcon} alt={m.work_speed()} class="h-6 w-6" />
			<span class="text-sm font-bold">{pal.rank_craftspeed}</span>
		</button>
	</div>
	<SectionHeader text={c.activeSkills} />
	<div
		style="opacity: {activeProgress.current}; transition: opacity 300ms ease-out;"
		class="flex w-full flex-col space-y-2"
	>
		{#each activeSkillsToShow as skill}
			<ActiveSkillBadge
				{skill}
				{pal}
				onSkillUpdate={handleUpdateActiveSkill}
			/>
		{/each}
	</div>
	<SectionHeader text={c.passiveSkills} />
	<div
		class="grid grid-cols-2 gap-2"
		style="opacity: {passiveProgress.current}; transition: opacity 300ms ease-out;"
	>
		{#each passiveSkillsToShow as skill}
			<PassiveSkillBadge
				{skill}
				{pal}
				onSkillUpdate={handleUpdatePassiveSkill}
			/>
		{/each}
	</div>
	<span class="text-justify">{palData?.description}</span>
</div>
