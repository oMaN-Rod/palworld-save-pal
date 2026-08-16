<script lang="ts">
	import { palsData, workSuitabilityData, WORK_SUITABILITY_KEYS } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { suitabilityImageMap } from '$utils/pals';
	import { toSlug } from '$lib/utils/wikiSlug';
	import type { PalData, WorkSuitability } from '$types';
	import WikiCard from './WikiCard.svelte';

	let selected = $state<string | null>(null);

	const selectedKey = $derived(selected ?? WORK_SUITABILITY_KEYS[0] ?? null);

	function workIcon(key: string): string {
		const id = suitabilityImageMap[key as WorkSuitability];
		if (!id) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${id}.webp`) as string;
	}

	function workLabel(key: string): string {
		return workSuitabilityData.workSuitability[key as WorkSuitability]?.localized_name || key;
	}

	function workDescription(key: string): string | null {
		const desc = workSuitabilityData.workSuitability[key as WorkSuitability]?.description;
		return typeof desc === 'string' && desc.length > 0 ? desc : null;
	}

	type PalLink = { key: string; pal: PalData; level: number };
	const palsForWork = $derived.by((): PalLink[] => {
		if (!selectedKey) return [];
		return Object.entries(palsData.pals)
			.map(([key, pal]) => {
				const level = (pal as PalData).work_suitability?.[selectedKey as WorkSuitability] ?? 0;
				return { key, pal: pal as PalData, level };
			})
			.filter(({ level }) => level > 0)
			.sort((a, b) => b.level - a.level);
	});

	function palIcon(key: string, pal: PalData) {
		const src = assetLoader.loadPalImage(key, pal.is_pal ?? true);
		return src ? { src } : null;
	}
</script>

<div class="flex flex-col gap-4">
	<!-- Tile row -->
	<div class="flex flex-wrap gap-2">
		{#each WORK_SUITABILITY_KEYS as key (key)}
			<button
				type="button"
				class="hover:border-primary-500/70 group flex h-20 w-20 flex-col items-center justify-center gap-1 rounded-lg border p-2 transition-colors {selectedKey === key ? 'border-primary-500 bg-surface-800' : 'border-surface-800'}"
				onclick={() => (selected = key)}
			>
				<img src={workIcon(key)} alt={key} class="h-8 w-8 object-contain" />
				<span class="text-surface-200 group-hover:text-surface-50 line-clamp-2 text-center text-xs leading-tight">{workLabel(key)}</span>
			</button>
		{/each}
	</div>

	<!-- Detail panel -->
	{#if selectedKey}
		<div class="border-surface-800 rounded-lg border p-4">
			<div class="mb-4 flex items-center gap-3">
				<img src={workIcon(selectedKey)} alt={selectedKey} class="h-12 w-12 object-contain" />
				<div class="flex flex-col">
					<h2 class="text-lg font-semibold">{workLabel(selectedKey)}</h2>
					{#if workDescription(selectedKey)}
						<p class="text-surface-300 text-sm">{workDescription(selectedKey)}</p>
					{/if}
				</div>
			</div>

			<div class="mb-2 text-xs text-surface-400">
				{palsForWork.length} pal{palsForWork.length === 1 ? '' : 's'} with this work suitability
			</div>
			<div class="grid max-h-[50vh] grid-cols-1 gap-2 overflow-y-auto pr-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
				{#each palsForWork as { key, pal, level } (key)}
					<WikiCard
						href="/wiki/pals/{toSlug(key)}"
						name={pal.localized_name}
						icon={palIcon(key, pal)}
					>
						{#snippet badges()}
							<span class="text-primary-400 text-xs font-semibold">Lv {level}</span>
						{/snippet}
					</WikiCard>
				{/each}
			</div>
		</div>
	{/if}
</div>
