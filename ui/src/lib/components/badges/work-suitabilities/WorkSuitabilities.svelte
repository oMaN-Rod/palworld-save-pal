<script lang="ts">
	import type { Pal, WorkSuitability } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { Tooltip } from '$components/ui';
	import { palsData } from '$lib/data';
	import { assetLoader } from '$utils';

	let { pal = $bindable() }: { pal: Pal | undefined } = $props();

	const suitabilityMap = {
		EmitFlame: 'kindling',
		Watering: 'watering',
		Seeding: 'planting',
		GenerateElectricity: 'generating',
		Handcraft: 'handiwork',
		Collection: 'gathering',
		Deforest: 'deforesting',
		Mining: 'mining',
		OilExtraction: 'extracting',
		ProductMedicine: 'production',
		Cool: 'cooling',
		Transport: 'transporting',
		MonsterFarm: 'farming'
	};

	let workSuitabilities = $derived.by(() => {
		if (pal) {
			const palData = Object.values(palsData.pals).find(
				(p) => p.localized_name.toLowerCase() === pal.name.toLowerCase()
			);
			if (palData) {
				return palData.work_suitability;
			}
		}
	});

	function loadIconPath(ws: WorkSuitability, value: number): string {
		const active = value >= 1;
		const prefix = active ? '' : 'no_';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/${prefix}${suitabilityMap[ws]}.png`);
	}

	function getFormattedName(suitability: WorkSuitability): string {
		return (
			suitabilityMap[suitability].charAt(0).toUpperCase() + suitabilityMap[suitability].slice(1)
		);
	}
</script>

<div class="grid w-full grid-cols-6 gap-2">
	{#if workSuitabilities}
		{#each Object.entries(workSuitabilities) as [ws, value]}
			{@const suitability: WorkSuitability = ws as WorkSuitability}
			{@const iconPath = loadIconPath(suitability, value)}
			<Tooltip>
				<div
					class="border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none {value ===
					0
						? 'text-[#646464]'
						: ''}"
				>
					<div class="flex w-full items-center">
						<img src={iconPath} alt="{suitability} icon" class="ml-2 h-6 w-6" />
						<span class="p-2 text-lg font-bold">{value}</span>
					</div>
				</div>
				{#snippet popup()}
					<span class="flex-grow p-2 text-lg">{getFormattedName(suitability)}</span>
				{/snippet}
			</Tooltip>
		{/each}
	{/if}
</div>
