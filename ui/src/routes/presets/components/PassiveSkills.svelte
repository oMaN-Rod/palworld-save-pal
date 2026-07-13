<script lang="ts">
	import { passiveSkillsData } from '$lib/data';
	import type { PresetProfile } from '$types';
	import { ASSET_DATA_PATH } from '$types/icons';
	import { assetLoader, skillFilter } from '$utils';

	let { preset = $bindable() } = $props<{
		preset: PresetProfile;
	}>();

	let passiveSkillIcons = $derived.by(() => {
		const icons: Record<string, string> = {};
		for (const skill of Object.values(passiveSkillsData.passiveSkills)) {
			if (icons[skill.details.rank]) continue;
			icons[skill.details.rank] = assetLoader.loadImage(
				`${ASSET_DATA_PATH}/img/rank_${skill.details.rank}.webp`
			) as string;
		}
		return icons;
	});

	function getPassiveSkillIconFilter(skillId: string): string {
		const skill = passiveSkillsData.getByKey(skillId);
		if (!skill || skill.localized_name === 'None') return '';
		return skillFilter(skill.details.rank);
	}
</script>

<div class="grid grid-cols-[20%_1fr] gap-2 rounded-sm p-4">
	<span class="border-r-surface-600 border-r pr-2 font-bold">{preset.name}</span>
	<div class="mt-1 ml-4 grid grid-cols-2 gap-2">
		{#each preset.skills as skillId}
			{@const skill = passiveSkillsData.getByKey(skillId)}
			{#if skill}
				<div class="flex items-center space-x-2">
					{#if passiveSkillIcons[skill.details.rank]}
						<img
							src={passiveSkillIcons[skill.details.rank]}
							alt={`Rank ${skill.details.rank}`}
							class="h-10 w-10"
							style="filter: {getPassiveSkillIconFilter(skillId)};"
						/>
					{/if}
					<span>{skill.localized_name}</span>
				</div>
			{:else}
				<span>{skillId}</span>
			{/if}
		{/each}
	</div>
</div>
