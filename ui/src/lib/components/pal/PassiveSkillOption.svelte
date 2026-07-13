<script lang="ts">
	import { passiveSkillsData } from '$lib/data';
	import { type SelectOption } from '$types';
	import { ASSET_DATA_PATH } from '$types/icons';
	import { assetLoader, skillFilter } from '$utils';

	let { option } = $props<{
		option: SelectOption;
	}>();

	const passiveSkill = passiveSkillsData.getByKey(option.value);
	const icon = $derived(
		assetLoader.loadImage(`${ASSET_DATA_PATH}/img/rank_${passiveSkill?.details.rank}.webp`)
	);
	const filter = $derived(skillFilter(passiveSkill?.details.rank ?? 1));
</script>

<div class="flex flex-row">
	<div class="flex grow flex-col">
		<span class="grow truncate">{option.label}</span>
		<span class="text-xs">{passiveSkill?.description}</span>
	</div>
	<img src={icon} alt={option.label} class="h-6 w-6" style="filter: {filter};" />
</div>
