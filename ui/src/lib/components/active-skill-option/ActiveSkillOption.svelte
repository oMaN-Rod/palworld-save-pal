<script lang="ts">
	import { activeSkillsData, elementsData } from '$lib/data';
	import { type SelectOption } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$types/icons';
	import { assetLoader } from '$utils';
	import { TimerReset } from 'lucide-svelte';

	let { option } = $props<{
		option: SelectOption;
	}>();

	const activeSkill = activeSkillsData.getByKey(option.value);
	const icon = $derived.by(() => {
		if (!activeSkill) return staticIcons.unknownIcon;
		const element = elementsData.elements[activeSkill.details.element];
		if (!element) return undefined;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${element.icon}.webp`);
	});
</script>

<div class="grid w-full grid-cols-[auto_1fr_auto] items-center gap-2">
	<img src={icon} alt={activeSkill?.localized_name} class="h-6 w-6" />
	<div class="mr-0.5 flex flex-col">
		<span class="truncate">{activeSkill?.localized_name}</span>
		<span class="text-xs">{activeSkill?.description}</span>
	</div>
	<div class="flex flex-col">
		<div class="flex items-center justify-end space-x-1">
			<TimerReset class="h-4 w-4" />
			<span class="font-bold">{activeSkill?.details.cool_time}</span>
			<span class="text-xs">Pwr</span>
			<span class="font-bold">{activeSkill?.details.power}</span>
		</div>
		<div class="flex flex-row items-center space-x-2">
			<div class="text-start">
				<span class="text-xs">{activeSkill?.details.type} Range</span>
				<span class="font-bold">
					{activeSkill?.details.min_range} - {activeSkill?.details.max_range}
				</span>
			</div>
		</div>
	</div>
</div>
