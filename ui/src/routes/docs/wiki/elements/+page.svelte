<script lang="ts">
	import { elementsData, palsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import type { ElementType } from '$types';

	let selectedKey = $state<string | null>(null);

	const allElements = $derived(Object.entries(elementsData.elements));

	const selectedElement = $derived(selectedKey ? elementsData.elements[selectedKey] : null);

	function getIcon(iconName: string): string {
		if (!iconName) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${iconName}.webp`) as string;
	}

	function getPalsWithElement(
		element: ElementType
	): { name: string; characterKey: string; elements: ElementType[] }[] {
		const pals = Object.entries(palsData.pals)
			.filter(
				([, pal]) => pal.is_pal && !pal.disabled && pal.element_types?.includes(element)
			)
			// canonical (shortest) key first so variants dedupe onto the base pal
			.sort(([a], [b]) => a.length - b.length || a.localeCompare(b))
			.map(([characterKey, pal]) => ({
				name: pal.localized_name,
				characterKey,
				elements: pal.element_types
			}));
		// Variants (Predator/Raid/Summon copies) share a display name; show one.
		const seen = new Set<string>();
		return pals
			.filter((p) => (seen.has(p.name) ? false : (seen.add(p.name), true)))
			.sort((a, b) => a.name.localeCompare(b.name));
	}
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="flex-1 overflow-y-auto">
			{#each allElements as [key, element] (key)}
				<button
					class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedKey ===
					key
						? 'bg-surface-700 text-surface-50'
						: 'text-surface-300 hover:bg-surface-800'}"
					onclick={() => (selectedKey = key)}
				>
					<img src={getIcon(element.icon)} alt="" class="h-5 w-5 shrink-0" />
					<span class="font-medium">{element.localized_name}</span>
				</button>
			{/each}
		</div>
	</div>

	<div class="border-surface-800 flex-1 overflow-y-auto rounded-lg border p-5">
		{#if selectedElement && selectedKey}
			<div class="flex items-center gap-4">
				<img src={getIcon(selectedElement.icon)} alt="" class="h-12 w-12" />
				<div>
					<h2 class="text-2xl font-bold" style="color: {selectedElement.color}">
						{selectedElement.localized_name}
					</h2>
					<span class="text-surface-400 text-sm">{selectedKey}</span>
				</div>
			</div>

			<div class="mt-6">
				<h3 class="text-surface-400 mb-3 text-sm font-semibold">Egg & Fruit</h3>
				<div class="grid grid-cols-3 gap-4">
					<div class="bg-surface-900 flex flex-col items-center gap-2 rounded-md p-4">
						<img src={getIcon(selectedElement.egg_icon)} alt="egg" class="h-24 w-24" />
						<span class="text-surface-400 text-xs">Egg</span>
					</div>
					<div class="bg-surface-900 flex flex-col items-center gap-2 rounded-md p-4">
						<img src={getIcon(selectedElement.fruit_icon)} alt="fruit" class="h-24 w-24" />
						<span class="text-surface-400 text-xs">Fruit</span>
					</div>
				</div>
			</div>

			{@const pals = getPalsWithElement(selectedKey as ElementType)}
			<div class="mt-6">
				<h3 class="text-surface-400 mb-2 text-sm font-semibold">Pals ({pals.length})</h3>
				<div class="grid grid-cols-1 gap-1 sm:grid-cols-2 lg:grid-cols-3">
					{#each pals as pal (pal.characterKey)}
						<div
							class="bg-surface-900 flex items-center gap-2 rounded-md px-3 py-1.5 text-sm"
						>
							<img
								src={assetLoader.loadMenuImage(pal.characterKey, true)}
								alt={`${pal.name} icon`}
								class="h-12 w-12 shrink-0 rounded-md object-cover"
							/>
							<span class="flex-1 text-left">{pal.name}</span>
							<div class="flex shrink-0 items-center gap-1">
								{#each pal.elements as el (el)}
									{@const elData = elementsData.elements[el]}
									{#if elData}
										<img
											src={getIcon(elData.icon)}
											alt={el}
											title={elData.localized_name}
											class="h-4 w-4"
										/>
									{/if}
								{/each}
							</div>
						</div>
					{/each}
				</div>
			</div>
		{:else}
			<div class="text-surface-500 flex h-full items-center justify-center">
				<p>Select an element to view details</p>
			</div>
		{/if}
	</div>
</div>