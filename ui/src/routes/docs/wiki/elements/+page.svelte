<script lang="ts">
	import { elementsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';

	let selectedKey = $state<string | null>(null);

	const allElements = $derived(Object.entries(elementsData.elements));

	const selectedElement = $derived(selectedKey ? elementsData.elements[selectedKey] : null);

	function getIcon(iconName: string): string {
		if (!iconName) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${iconName}.webp`) as string;
	}
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<h1 class="mb-3 text-lg font-bold">Elements</h1>
		<div class="flex-1 overflow-y-auto">
			{#each allElements as [key, element]}
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
				<img src={getIcon(selectedElement.icon)} alt="" class="h-16 w-16" />
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
		{:else}
			<div class="text-surface-500 flex h-full items-center justify-center">
				<p>Select an element to view details</p>
			</div>
		{/if}
	</div>
</div>
