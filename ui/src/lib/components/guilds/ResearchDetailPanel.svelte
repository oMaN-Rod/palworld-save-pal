<script lang="ts">
	import { Card } from '$components/ui';
	import { itemsData } from '$lib/data';
	import type { TreeNode } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$types/icons';
	import { assetLoader } from '$utils';
	import * as m from '$i18n/messages';

	let { selectedNode }: { selectedNode: TreeNode } = $props();
</script>

<Card class="m-4 h-auto rounded-lg">
	{@const research = selectedNode.research}
	<h5 class="h5 mb-2">{research.localized_name}</h5>

	{#if research.details.effect_type && research.details.effect_type !== 'None'}
		<div class="mb-4">
			<h6 class="h6 mb-1">{m.effect()}</h6>
			<span class="text-sm">
				{research.details.effect_type}: {research.details.effect_value &&
				research.details.effect_value > 0
					? '+'
					: ''}{research.details.effect_value ?? 0}%
				{#if research.details.effect_work_suitability && research.details.effect_work_suitability !== 'None'}
					{m.for()} {research.details.effect_work_suitability}
				{/if}
				{#if research.details.effect_item_type && research.details.effect_item_type !== 'None'}
					{m.for()} {research.details.effect_item_type}
				{/if}
			</span>
		</div>
	{/if}

	{#if research.details.materials && research.details.materials.length > 0}
		<h6 class="h6 mb-1">{m.research_cost()}</h6>
		<div class="space-y-1">
			{#each research.details.materials as material}
				{@const itemData = itemsData.getByKey(material.id)}
				<div class="flex items-center space-x-2 text-sm">
					{#if itemData}
						{@const icon = assetLoader.loadImage(
							`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`
						)}
						<img
							src={icon || staticIcons.unknownIcon}
							alt={itemData.info.localized_name}
							class="h-5 w-5"
						/>
						<span>{itemData.info.localized_name}</span>
					{:else}
						<img src={staticIcons.unknownIcon} alt={material.id} class="h-5 w-5" />
						<span>{material.id}</span>
					{/if}
					<span class="ml-auto">{material.count}</span>
				</div>
			{/each}
			<div class="border-surface-600 flex items-center space-x-2 border-t pt-2 text-sm">
				<img src={staticIcons.workSpeedIcon} alt="Workload" class="h-5 w-5" />
				<span>{m.workload()}</span>
				<span class="ml-auto">{research.details.work_amount}</span>
			</div>
		</div>
	{/if}
</Card>