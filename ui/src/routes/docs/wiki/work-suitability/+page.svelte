<script lang="ts">
	import { workSuitabilityData, palsData } from '$lib/data';
	import type { WorkSuitability } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader, suitabilityImageMap } from '$utils';

	let selectedKey = $state<string | null>(null);

	const allWorkTypes = $derived(
		Object.entries(workSuitabilityData.workSuitability) as [
			WorkSuitability,
			{ localized_name: string; description: string }
		][]
	);

	const selectedWork = $derived.by(() => {
		if (!selectedKey) return null;
		const entry = allWorkTypes.find(([key]) => key === selectedKey);
		return entry ? entry[1] : null;
	});

	function getWorkIcon(workType: WorkSuitability): string {
		const base = suitabilityImageMap[workType];
		if (!base) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${base}.webp`) as string;
	}

	function getPalsWithWork(
		workType: WorkSuitability
	): { name: string; level: number; characterKey: string }[] {
		const pals = Object.entries(palsData.pals)
			.filter(([, pal]) => pal.is_pal && !pal.disabled && pal.work_suitability[workType] > 0)
			// canonical (shortest) key first so variants dedupe onto the base pal
			.sort(([a], [b]) => a.length - b.length || a.localeCompare(b))
			.map(([characterKey, pal]) => ({
				name: pal.localized_name,
				level: pal.work_suitability[workType],
				characterKey
			}));
		// Variants share a display name; keep the highest-ranked entry.
		const best = new Map<string, { name: string; level: number; characterKey: string }>();
		for (const p of pals) {
			const prev = best.get(p.name);
			if (!prev || p.level > prev.level) best.set(p.name, p);
		}
		return [...best.values()].sort((a, b) => b.level - a.level);
	}
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="flex-1 overflow-y-auto">
			{#each allWorkTypes as [key, data] (key)}
				<button
					class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedKey ===
					key
						? 'bg-surface-700 text-surface-50'
						: 'text-surface-300 hover:bg-surface-800'}"
					onclick={() => (selectedKey = key)}
				>
					<img src={getWorkIcon(key)} alt="" class="h-5 w-5 shrink-0" />
					<span class="font-medium">{data.localized_name || key}</span>
				</button>
			{/each}
		</div>
	</div>

	<div class="border-surface-800 flex-1 overflow-y-auto rounded-lg border p-5">
		{#if selectedWork && selectedKey}
			<div class="flex items-center gap-3">
				<img
					src={getWorkIcon(selectedKey as WorkSuitability)}
					alt=""
					class="h-12 w-12 shrink-0"
				/>
				<h2 class="text-2xl font-bold">{selectedWork.localized_name || selectedKey}</h2>
			</div>
			{#if selectedWork.description}
				<p class="text-surface-300 mt-2">{selectedWork.description}</p>
			{/if}

			{@const pals = getPalsWithWork(selectedKey as WorkSuitability)}
			<div class="mt-5">
				<h3 class="text-surface-400 mb-2 text-sm font-semibold">Pals ({pals.length})</h3>
				<div class="grid grid-cols-1 gap-1 sm:grid-cols-2 lg:grid-cols-3">
					{#each pals as pal (pal.characterKey)}
						<div
							class="bg-surface-900 flex items-center justify-between gap-2 rounded-md px-3 py-1.5 text-sm"
						>
							<img
								src={assetLoader.loadMenuImage(pal.characterKey, true)}
								alt={`${pal.name} icon`}
								class="h-12 w-12 shrink-0 rounded-md object-cover"
							/>
							<span class="flex-1 text-left">{pal.name}</span>
							<span class="text-surface-400 font-semibold">Lv.{pal.level}</span>
						</div>
					{/each}
				</div>
			</div>
		{:else}
			<div class="text-surface-500 flex h-full items-center justify-center">
				<p>Select a work type to view details</p>
			</div>
		{/if}
	</div>
</div>