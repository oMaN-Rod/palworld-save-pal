<script lang="ts">
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { elementsData, palsData } from '$lib/data';
	import { assetLoader, calculateFilters } from '$utils';
	import { staticIcons } from '$types/icons';
	import { User } from 'lucide-svelte';
	import type { ElementType, Pal, PalData } from '$types';
	import { getAppState } from '$states';
	import * as m from '$i18n/messages';
	import { c, p } from '$lib/utils/commonTranslations';

	type PalWithData = {
		id: string;
		pal: Pal;
		palData?: PalData;
	};

	let {
		pals,
		elementTypes,
		elementsData: elements
	} = $props<{
		pals: PalWithData[];
		elementTypes: string[];
		elementsData?: any;
	}>();

	// Fallback for elementsData if not provided
	elements = elements || elementsData;

	const appState = getAppState();

	// Load element icons
	let elementIcons = $derived.by(() => {
		let icons: Record<string, string> = {};
		for (const element of elementTypes) {
			const elementData = elements.elements[element];
			if (elementData) {
				icons[element] = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/${elementData.icon}.webp`
				) as string;
			}
		}
		return icons;
	});

	// Element type statistics
	let elementStats = $derived.by(() => {
		if (!pals || pals.length === 0) return {};

		const stats: Record<string, number> = {};

		// Initialize all element types with zero counts
		elementTypes.forEach((element: string) => {
			stats[element] = 0;
		});

		// Count pals by element type
		pals.forEach(({ palData }: PalWithData) => {
			if (palData && palData.element_types) {
				palData.element_types.forEach((elementType: ElementType) => {
					const element = elementType.toString();
					stats[element] = (stats[element] || 0) + 1;
				});
			}
		});

		return stats;
	});

	// Special categories statistics
	let specialStats = $derived.by(() => {
		if (!pals || pals.length === 0)
			return { alpha: 0, lucky: 0, human: 0, predator: 0, oilrig: 0, summon: 0 };

		return {
			alpha: pals.filter(({ pal }: PalWithData) => pal.is_boss).length,
			lucky: pals.filter(({ pal }: PalWithData) => pal.is_lucky).length,
			human: pals.filter(({ palData }: PalWithData) => palData && !palData.is_pal).length,
			predator: pals.filter(({ pal }: PalWithData) =>
				pal.character_id.toLowerCase().includes('predator_')
			).length,
			oilrig: pals.filter(({ pal }: PalWithData) =>
				pal.character_id.toLowerCase().includes('_oilrig')
			).length,
			summon: pals.filter(({ pal }: PalWithData) =>
				pal.character_id.toLowerCase().includes('summon_')
			).length
		};
	});

	// Level statistics
	let levelStats = $derived.by(() => {
		if (!pals || pals.length === 0) {
			return { average: 0, max: 0, min: 0, maxCount: 0 };
		}

		const levels = pals
			.map(({ pal }: PalWithData) => pal.level || 0)
			.filter((level: number) => level > 0);

		if (levels.length === 0) {
			return { average: 0, max: 0, min: 0, maxCount: 0 };
		}

		const max = Math.max(...levels);
		const maxLevel = appState.settings.cheat_mode ? 255 : 60;

		return {
			average: levels.reduce((sum: number, level: number) => sum + level, 0) / levels.length,
			max,
			min: Math.min(...levels),
			maxCount: levels.filter((level: number) => level === maxLevel).length
		};
	});

	// Total count
	let totalPals = $derived(pals.length);
</script>

<div class="flex flex-col space-y-3">
	<!-- Overall stats -->
	<div class="flex space-x-2">
		<span class="grow font-bold">{m.total_pals(p.pals)}</span>
		<span>{totalPals}</span>
	</div>

	<!-- Element type stats -->
	<div>
		<h5 class="mb-1 font-bold">{m.elemental_distribution()}</h5>
		<div class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
			{#each elementTypes as element}
				{@const count = elementStats[element] || 0}
				<div class="flex items-center">
					<img src={elementIcons[element]} alt={element} class="pal-element-badge mr-2" />
					<div class="grow">
						<span class="text-xs 2xl:text-base">
							{elements.elements[element]?.localized_name || element}
						</span>
					</div>
					<span>{count}</span>
				</div>
			{/each}
		</div>
	</div>

	<!-- Special categories -->
	<div>
		<h5 class="mb-1 font-bold">{m.special_categories()}</h5>
		<div class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
			<div class="flex items-center">
				<img src={staticIcons.alphaIcon} alt="Alpha" class="pal-element-badge mr-2" />
				<div class="grow">
					<span class="text-xs 2xl:text-base">{m.alpha()}</span>
				</div>
				<span>{specialStats.alpha}</span>
			</div>
			<div class="flex items-center">
				<img src={staticIcons.luckyIcon} alt="Lucky" class="pal-element-badge mr-2" />
				<div class="grow">
					<span class="text-xs 2xl:text-base">{m.lucky()}</span>
				</div>
				<span>{specialStats.lucky}</span>
			</div>
			<div class="flex items-center">
				<User class="pal-element-badge mr-2" />
				<div class="grow">
					<span class="text-xs 2xl:text-base">{c.human}</span>
				</div>
				<span>{specialStats.human}</span>
			</div>
			<div class="flex items-center">
				<img
					src={staticIcons.predatorIcon}
					alt="Predator"
					class="pal-element-badge mr-2"
					style="filter: {calculateFilters('#FF0000')}"
				/>
				<div class="grow">
					<span class="text-xs 2xl:text-base">{m.predator()}</span>
				</div>
				<span>{specialStats.predator}</span>
			</div>
			<div class="flex items-center">
				<img src={staticIcons.oilrigIcon} alt="Oil Rig" class="pal-element-badge mr-2" />
				<div class="grow">
					<span class="text-xs 2xl:text-base">{m.oil_rig()}</span>
				</div>
				<span>{specialStats.oilrig}</span>
			</div>
			<div class="flex items-center">
				<img src={staticIcons.altarIcon} alt="Summoned" class="pal-element-badge mr-2" />
				<div class="grow">
					<span class="text-xs 2xl:text-base">{m.summoned()}</span>
				</div>
				<span>{specialStats.summon}</span>
			</div>
		</div>
	</div>

	<!-- Level stats -->
	<div>
		<h5 class="mb-1 font-bold">{m.level_distribution()}</h5>
		<div class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
			<div class="flex space-x-2">
				<div class="grow">
					<span class="text-xs 2xl:text-base">{m.avg_level()}</span>
				</div>
				<span>{levelStats.average.toFixed(1)}</span>
			</div>
			<div class="flex space-x-2">
				<div class="grow">
					<span class="text-xs 2xl:text-base">{m.max_level()}</span>
				</div>
				<span>{levelStats.max}</span>
			</div>
			<div class="flex space-x-2">
				<div class="grow">
					<span class="text-xs 2xl:text-base">{m.min_level()}</span>
				</div>
				<span>{levelStats.min}</span>
			</div>
			<div class="flex space-x-2">
				<div class="grow">
					<span class="text-xs 2xl:text-base">{m.max_level_pals(p.pals)}</span>
				</div>
				<span>{levelStats.maxCount}</span>
			</div>
		</div>
	</div>
</div>

<style>
	.pal-element-badge {
		width: 24px;
		height: 24px;
	}
</style>
