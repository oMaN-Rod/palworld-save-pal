<script lang="ts">
	import { EntryState, type Pal, type WorkSuitability } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { Tooltip } from '$components/ui';
	import { palsData, workSuitabilityData } from '$lib/data';
	import { assetLoader } from '$utils';
	import { NumberSliderModal } from '$components/modals';
	import { getModalState } from '$states';
	import * as m from '$i18n/messages';

	const modal = getModalState();

	let { pal = $bindable() }: { pal: Pal | undefined } = $props();

	const suitabilityImageMap = {
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
			let suitability: Record<WorkSuitability, number> = {
				EmitFlame: 0,
				Watering: 0,
				Seeding: 0,
				GenerateElectricity: 0,
				Handcraft: 0,
				Collection: 0,
				Deforest: 0,
				Mining: 0,
				OilExtraction: 0,
				ProductMedicine: 0,
				Cool: 0,
				Transport: 0,
				MonsterFarm: 0
			};
			const palData = palsData.getByKey(pal.character_key);
			if (palData) {
				for (const [key, value] of Object.entries(palData.work_suitability)) {
					const palScaledSuitability = pal.work_suitability[key as WorkSuitability] ?? 0;
					suitability[key as WorkSuitability] = value + palScaledSuitability;
				}
			}
			return suitability;
		}
	});

	function loadIconPath(ws: WorkSuitability, value: number): string {
		const active = value >= 1;
		const prefix = active ? '' : 'no_';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${prefix}${suitabilityImageMap[ws]}.webp`);
	}

	function getFormattedName(suitability: WorkSuitability): string {
		return (
			suitabilityImageMap[suitability].charAt(0).toUpperCase() +
			suitabilityImageMap[suitability].slice(1)
		);
	}

	async function handleEditSuitability(workSuitability: string, value: number): Promise<void> {
		// @ts-ignore
		const result = await modal.showModal<number>(NumberSliderModal, {
			title: m.edit_entity({ entity: m.work_suitability() }),
			value: value,
			min: 0,
			max: 5,
			markers: [1, 2, 3, 4, 5]
		});
		if (!result) return;
		if (!pal!.work_suitability) {
			pal!.work_suitability = {} as Record<WorkSuitability, number>;
		}
		const palData = palsData.getByKey(pal!.character_key);
		pal!.work_suitability = {
			...pal!.work_suitability,
			[workSuitability]: Math.min(
				result - (palData?.work_suitability[workSuitability as WorkSuitability] ?? 0),
				4
			)
		};

		pal!.state = EntryState.MODIFIED;
	}
</script>

<div class="grid w-full grid-cols-6 gap-2">
	{#if workSuitabilities}
		{#each Object.entries(workSuitabilities) as [ws, value]}
			{@const suitability: WorkSuitability = ws as WorkSuitability}
			{@const iconPath = loadIconPath(suitability, value)}
			<Tooltip>
				<button
					class="border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none {value ===
					0
						? 'text-[#646464]'
						: ''}"
					disabled={value === 0}
					onclick={() => handleEditSuitability(ws, value)}
				>
					<div class="flex w-full items-center">
						<img src={iconPath} alt="{suitability} icon" class="ml-2 h-4 w-4 2xl:h-6 2xl:w-6" />
						<span class="p-2 text-sm font-bold 2xl:text-lg">{value}</span>
					</div>
				</button>
				{#snippet popup()}
					<div class="flex items-center space-x-2">
						{#if value !== 0}
							<span class="p-2 text-lg font-bold">{m.level_abbr_value({ value })}</span>
						{/if}
						<span class="text-lg">
							{workSuitabilityData.workSuitability[suitability].localized_name ??
								getFormattedName(suitability)}
						</span>
					</div>
				{/snippet}
			</Tooltip>
		{/each}
	{/if}
</div>
