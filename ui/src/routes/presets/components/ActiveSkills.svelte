<script lang="ts">
	import { activeSkillsData, elementsData } from '$lib/data';
	import type { PresetProfile } from '$types';
	import { ASSET_DATA_PATH } from '$types/icons';
	import { assetLoader } from '$utils';

	let { preset = $bindable() } = $props<{
		preset: PresetProfile;
	}>();

	let elementIcons = $derived.by(() => {
		const icons: Record<string, string> = {};
		for (const elementType of Object.keys(elementsData.elements)) {
			const elementObj = elementsData.elements[elementType];
			if (elementObj) {
				icons[elementType] = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/${elementObj.badge_icon}.webp`
				) as string;
			}
		}
		return icons;
	});
</script>

<div class="grid grid-cols-[20%_1fr] gap-2 rounded-sm p-4">
	<span class="border-r-surface-600 border-r pr-2 text-lg font-bold">{preset.name}</span>
	<div class="ml-4 mt-1 space-y-4">
		{#each preset.skills as skillId}
			{@const skill = activeSkillsData.getByKey(skillId)}
			{#if skill}
				<div class="flex items-center space-x-2">
					{#if skill.details.element && elementIcons[skill.details.element]}
						<img
							src={elementIcons[skill.details.element]}
							alt={skill.details.element}
							class="h-10 w-10"
						/>
					{/if}
					<span>{skill.localized_name}</span>
					<span class="text-surface-400"
						>({skill.details.power === 0 ? 'NA' : skill.details.power})</span
					>
				</div>
			{:else}
				<span>{skillId}</span>
			{/if}
		{/each}
	</div>
</div>
