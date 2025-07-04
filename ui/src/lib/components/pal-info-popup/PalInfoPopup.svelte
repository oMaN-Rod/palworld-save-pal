<script lang="ts">
	import { SectionHeader } from '$components/ui';
	import { type Pal } from '$types';
	import { getAppState } from '$states';
	import { ActiveSkillBadge, StatusBadge, PalHeader, PassiveSkillBadge } from '$components';
	import { palsData } from '$lib/data';
	import { staticIcons } from '$types/icons';
	import { onMount } from 'svelte';
	import { Tween } from 'svelte/motion';
	import { cubicOut } from 'svelte/easing';

	let { pal = $bindable() } = $props<{
		pal: Pal;
	}>();

	const appState = getAppState();

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

	const palData = $derived(palsData.pals[pal.character_key]);

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
</script>

<div class="flex w-[450px] flex-col space-y-2">
	<PalHeader bind:pal showActions={false} popup />
	<StatusBadge bind:pal />
	<div class="flex justify-center space-x-2">
		<span class="text-surface-300 mr-1 text-xs">IVs</span>
		<div class="chip bg-green-700">
			<img src={staticIcons.hpIcon} alt="HP" class="h-4 w-4" />
			<span class="text-sm font-bold">{pal.talent_hp}</span>
		</div>
		<div class="chip bg-red-700">
			<img src={staticIcons.attackIcon} alt="Attack" class="h-6 w-6" />
			<span class="text-sm font-bold">{pal.talent_shot}</span>
		</div>
		<div class="chip bg-blue-700">
			<img src={staticIcons.defenseIcon} alt="Defense" class="h-6 w-6" />
			<span class="text-sm font-bold">{pal.talent_defense}</span>
		</div>
	</div>
	<div class="flex justify-center space-x-2">
		<span class="text-surface-300 mr-1 text-xs">Souls</span>
		<div class="chip bg-green-700">
			<img src={staticIcons.hpIcon} alt="HP" class="h-4 w-4" />
			<span class="text-sm font-bold">{pal.rank_hp}</span>
		</div>
		<div class="chip bg-red-700">
			<img src={staticIcons.attackIcon} alt="Attack" class="h-6 w-6" />
			<span class="text-sm font-bold">{pal.rank_attack}</span>
		</div>
		<div class="chip bg-blue-700">
			<img src={staticIcons.defenseIcon} alt="Defense" class="h-6 w-6" />
			<span class="text-sm font-bold">{pal.rank_defense}</span>
		</div>
		<div class="chip bg-purple-700">
			<img src={staticIcons.workSpeedIcon} alt="Defense" class="h-6 w-6" />
			<span class="text-sm font-bold">{pal.rank_craftspeed}</span>
		</div>
	</div>
	{#if activeSkillsToShow.length > 0}
		<SectionHeader text="Active Skills" />
		<div
			style="opacity: {activeProgress.current}; transition: opacity 300ms ease-out;"
			class="flex w-full flex-col space-y-2"
		>
			{#each activeSkillsToShow as skill}
				<ActiveSkillBadge {skill} />
			{/each}
		</div>
	{/if}
	{#if passiveSkillsToShow.length > 0}
		<SectionHeader text="Passive Skills" />
		<div
			class="grid grid-cols-2 gap-2"
			style="opacity: {passiveProgress.current}; transition: opacity 300ms ease-out;"
		>
			{#each passiveSkillsToShow as skill}
				<PassiveSkillBadge {skill} />
			{/each}
		</div>
	{/if}
	<span class="text-justify">{palData?.description}</span>
</div>
