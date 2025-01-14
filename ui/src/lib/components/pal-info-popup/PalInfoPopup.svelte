<script lang="ts">
	import { SectionHeader } from '$components/ui';
	import { type Pal } from '$types';
	import { getAppState } from '$states';
	import { ActiveSkillBadge, HealthBadge, PalHeader, PassiveSkillBadge } from '$components';
	import { palsData } from '$lib/data';

	let { pal = $bindable() } = $props<{
		pal: Pal;
	}>();

	const appState = getAppState();
	let activeSkills = $derived.by(() => {
		if (pal) {
			let skills = [...pal.active_skills];
			while (skills.length < 3) {
				skills.push('Empty');
			}
			return skills;
		} else {
			return [];
		}
	});

	let passiveSkills = $derived.by(() => {
		if (pal) {
			let skills = [...pal.passive_skills];
			while (skills.length < 4) {
				skills.push('Empty');
			}
			return skills;
		} else {
			return [];
		}
	});

	let palData = $derived(palsData.pals[pal.character_key]);
</script>

<div class="flex w-[450px] flex-col space-y-2">
	<PalHeader bind:pal showActions={false} />
	<HealthBadge bind:pal player={appState.selectedPlayer} />
	<div class="flex justify-center space-x-2">
		<div class="chip bg-green-700">
			<span class="text-xs font-light">HP</span>
			<span class="text-sm font-bold">{pal.talent_hp}</span>
		</div>
		<div class="chip bg-red-700">
			<span class="text-xs font-light">Attack</span>
			<span class="text-sm font-bold">{pal.talent_shot}</span>
		</div>
		<div class="chip bg-blue-700">
			<span class="text-xs font-light">Defense</span>
			<span class="text-sm font-bold">{pal.talent_defense}</span>
		</div>
	</div>
	<div class="flex justify-center space-x-2">
		<div class="chip bg-green-700">
			<span class="text-xs font-light">Health</span>
			<span class="text-sm font-bold">{pal.rank_hp}</span>
		</div>
		<div class="chip bg-red-700">
			<span class="text-xs font-light">Attack</span>
			<span class="text-sm font-bold">{pal.rank_attack}</span>
		</div>
		<div class="chip bg-blue-700">
			<span class="text-xs font-light">Defense</span>
			<span class="text-sm font-bold">{pal.rank_defense}</span>
		</div>
		<div class="chip bg-purple-700">
			<span class="text-xs font-light">Craftspeed</span>
			<span class="text-sm font-bold">{pal.rank_craftspeed}</span>
		</div>
	</div>
	{#if activeSkills.length > 0}
		<SectionHeader text="Active Skills" />
		{#each activeSkills as skill}
			<ActiveSkillBadge {skill} palCharacterId={pal.character_key} />
		{/each}
	{/if}
	{#if passiveSkills.length > 0}
		<SectionHeader text="Passive Skills" />
		<div class="grid grid-cols-2 gap-2">
			{#each passiveSkills as skill}
				<PassiveSkillBadge {skill} />
			{/each}
		</div>
	{/if}
	<span class="text-justify">{palData?.description}</span>
</div>
