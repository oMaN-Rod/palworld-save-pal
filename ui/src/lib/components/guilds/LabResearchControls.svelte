<script lang="ts">
	import { labResearchData, workSuitabilityData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';
	import { staticIcons } from '$types/icons';
	import type { Guild, GuildLabResearchInfo, WorkSuitability } from '$types';
	import NumberFlow from '@number-flow/svelte';
	import { Unlock } from 'lucide-svelte';
	import * as m from '$i18n/messages';

	let {
		guild = $bindable(),
		selectedCategory = $bindable(),
		unlockAllForCategory = $bindable()
	} = $props<{
		guild: Guild;
		selectedCategory: string;
		unlockAllForCategory: (category: string) => void;
	}>();

	const categories = $derived.by(() => {
		const uniqueCategories = [
			...new Set(Object.values(labResearchData.research).map((r) => r.details.category))
		].filter(Boolean);
		return uniqueCategories.sort((a, b) => {
			const order = [
				'Handcraft',
				'EmitFlame',
				'Watering',
				'Seeding',
				'GenerateElectricity',
				'Deforest',
				'Mining',
				'Cool',
				'ProductMedicine'
			];
			return order.indexOf(a as string) - order.indexOf(b as string);
		}) as string[];
	});

	const categoryIcons: Record<string, string> = {
		Handcraft: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/handiwork.webp`),
		EmitFlame: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/fire.webp`),
		Watering: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/watering.webp`),
		Seeding: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/planting.webp`),
		GenerateElectricity: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/generating.webp`),
		Deforest: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/deforesting.webp`),
		Mining: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/mining.webp`),
		Cool: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/ice.webp`),
		ProductMedicine: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/production.webp`)
	};

	function getCategoryProgress(category: string) {
		const researchItemsData = Object.entries(labResearchData.research)
			.filter(([_, item]) => item.details.category === category)
			.map(([key, _]) => key);
		const researchItems = (
			Object.values(guild?.lab_research_data || {}) as GuildLabResearchInfo[]
		).filter((item) => researchItemsData.includes(item.research_id));
		const totalItems = researchItemsData.length;
		let completedItems = 0;
		for (const item of researchItems) {
			const labItem = labResearchData.getByKey(item.research_id);
			if (!labItem) continue;
			const totalWorkAmount = labItem.details.work_amount || 0;
			const isCompleted = item.work_amount >= totalWorkAmount && totalWorkAmount > 0;
			if (isCompleted) {
				completedItems++;
			}
		}
		return {
			completed: completedItems,
			total: totalItems,
			progress: totalItems > 0 ? (completedItems / totalItems) * 100 : 0
		};
	}
</script>

<div class="flex flex-col space-y-1 overflow-y-auto">
	{#each categories as category}
		{@const iconSrc = categoryIcons[category] || staticIcons.unknownIcon}
		{@const workSuitability = workSuitabilityData.workSuitability[category as WorkSuitability]}
		{@const categoryProgress = getCategoryProgress(category)}
		<div class="flex items-center gap-2">
			<button
				class={cn(
					'btn border-surface-800 hover:ring-secondary-500 flex flex-1 items-center justify-start space-x-2 rounded-none border p-2 text-start text-sm hover:ring-2 hover:ring-inset',
					selectedCategory === category ? 'bg-secondary-800/50' : ''
				)}
				onclick={() => (selectedCategory = category)}
			>
				<img src={iconSrc} alt={category} class="h-8 w-8" />
				<span class="grow">
					{workSuitability.localized_name || category}
				</span>
				<div class="flex gap-2">
					<NumberFlow value={categoryProgress.completed} class="font-bold" />
					<span class="text-sm"> / {categoryProgress.total}</span>
				</div>
			</button>
			<button
				class="btn hover:ring-secondary-500 border-surface-800 flex items-center justify-center border p-2 text-sm hover:ring-2 hover:ring-inset"
				onclick={() => unlockAllForCategory(category)}
				title={m.unlock_all_category_research({
					category: workSuitability.localized_name || category
				})}
			>
				<Unlock class="h-4 w-4" />
			</button>
		</div>
	{/each}
</div>
