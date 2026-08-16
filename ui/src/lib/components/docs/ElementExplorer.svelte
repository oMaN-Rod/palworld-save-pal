<script lang="ts">
	import { elementsData, palsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { byPaldeckIndex } from '$lib/utils/wikiDescriptors';
	import { toSlug } from '$lib/utils/wikiSlug';
	import type { PalData } from '$types';
	import WikiCard from './WikiCard.svelte';

	let selected = $state<string | null>(null);

	const elementKeys = $derived(Object.keys(elementsData.elements));

	const selectedKey = $derived(selected ?? elementKeys[0] ?? null);

	function elementIcon(key: string): string {
		const el = elementsData.elements[key];
		if (!el) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${el.icon}.webp`) as string;
	}

	function elementExtras(key: string): { src: string; label: string }[] {
		const el = elementsData.elements[key];
		if (!el) return [];
		const out: { src: string; label: string }[] = [];
		for (const [field, label] of [
			['egg_icon', 'Egg'],
			['fruit_icon', 'Fruit']
		] as const) {
			const id = el[field as keyof typeof el];
			if (typeof id === 'string' && id.length > 0) {
				const src = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${id}.webp`) as string;
				if (src) out.push({ src, label });
			}
		}
		return out;
	}

	type PalLink = { key: string; pal: PalData };
	const palsWithElement = $derived.by((): PalLink[] => {
		if (!selectedKey) return [];
		return Object.entries(palsData.pals)
			.filter(([, pal]) => (pal as PalData).element_types?.includes(selectedKey as never))
			.map(([key, pal]) => ({ key, pal: pal as PalData }))
			.sort((a, b) =>
				byPaldeckIndex(a.pal.pal_deck_index, b.pal.pal_deck_index)
			);
	});

	function palIcon(key: string, pal: PalData) {
		const src = assetLoader.loadPalImage(key, pal.is_pal ?? true);
		return src ? { src } : null;
	}
</script>

<div class="flex flex-col gap-4">
	<!-- Puck row -->
	<div class="flex flex-wrap gap-3">
		{#each elementKeys as key (key)}
			{@const el = elementsData.elements[key]}
			{@const name = el.localized_name || key}
			<button
				type="button"
				class="hover:border-primary-500/70 group flex h-20 w-20 flex-col items-center justify-center gap-1 rounded-lg border p-2 transition-colors {selectedKey === key ? 'border-primary-500 bg-surface-800' : 'border-surface-800'}"
				onclick={() => (selected = key)}
			>
				<span
					class="flex h-10 w-10 items-center justify-center rounded-full"
					style={`background: ${el.color}22; box-shadow: inset 0 0 0 2px ${el.color};`}
				>
					<img src={elementIcon(key)} alt={name} class="h-6 w-6 object-contain" />
				</span>
				<span class="text-surface-200 group-hover:text-surface-50 truncate text-xs">{name}</span>
			</button>
		{/each}
	</div>

	<!-- Detail panel -->
	{#if selectedKey}
		{@const el = elementsData.elements[selectedKey]}
		{@const name = el.localized_name || selectedKey}
		<div class="border-surface-800 rounded-lg border p-4">
			<div class="mb-4 flex items-center gap-3">
				<span
					class="flex h-14 w-14 items-center justify-center rounded-full"
					style={`background: ${el.color}22; box-shadow: inset 0 0 0 2px ${el.color};`}
				>
					<img src={elementIcon(selectedKey)} alt={name} class="h-9 w-9 object-contain" />
				</span>
				<div class="flex flex-col">
					<h2 class="text-lg font-semibold">{name}</h2>
					<div class="flex items-center gap-2">
						<span class="inline-block h-3 w-3 rounded-full" style={`background: ${el.color};`}></span>
						<span class="text-surface-400 font-mono text-xs">{el.color}</span>
					</div>
				</div>
				{#each elementExtras(selectedKey) as extra (extra.label)}
					<div class="ml-2 flex flex-col items-center gap-1">
						<img src={extra.src} alt={extra.label} class="h-9 w-9 object-contain" />
						<span class="text-surface-400 text-xs">{extra.label}</span>
					</div>
				{/each}
			</div>

			<div class="mb-2 text-xs text-surface-400">
				{palsWithElement.length} pal{palsWithElement.length === 1 ? '' : 's'} with this element
			</div>
			<div class="grid max-h-[50vh] grid-cols-1 gap-2 overflow-y-auto pr-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
				{#each palsWithElement as { key, pal } (key)}
					<WikiCard
						href="/wiki/pals/{toSlug(key)}"
						name={pal.localized_name}
						icon={palIcon(key, pal)}
					>
						{#snippet badges()}
							{#if pal.pal_deck_index > 0}
								<span class="text-surface-400 text-xs">#{pal.pal_deck_index}</span>
							{/if}
						{/snippet}
					</WikiCard>
				{/each}
			</div>
		</div>
	{/if}
</div>
