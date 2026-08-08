<script lang="ts">
	import { palsData, elementsData } from '$lib/data';
	import { WikiGrid, WikiSearch } from '$components/docs';
	import { Tooltip } from '$components/ui';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { toSlug } from '$lib/utils/wikiSlug';
	import { byPaldeckIndex } from '$lib/utils/wikiDescriptors';
	import { cn } from '$theme';
	import { staticIcons } from '$types/icons';
	import ArrowDownAZ from '@lucide/svelte/icons/arrow-down-a-z';
	import ArrowDownZA from '@lucide/svelte/icons/arrow-down-z-a';
	import ArrowDownWideNarrow from '@lucide/svelte/icons/arrow-down-wide-narrow';
	import ArrowDownNarrowWide from '@lucide/svelte/icons/arrow-down-narrow-wide';
	import GalleryVerticalEnd from '@lucide/svelte/icons/gallery-vertical-end';
	import User from '@lucide/svelte/icons/user';

	type SortBy = 'name' | 'paldeck-index';
	type SortOrder = 'asc' | 'desc';

	let search = $state('');
	let selectedFilter = $state('All');
	let sortBy: SortBy = $state('paldeck-index');
	let sortOrder: SortOrder = $state('asc');

	const elementTypes = $derived(Object.keys(elementsData.elements));
	const elementIcons = $derived.by(() => {
		const icons: Record<string, string> = {};
		for (const element of elementTypes) {
			const elementData = elementsData.elements[element];
			if (elementData) {
				icons[element] = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/${elementData.icon}.webp`
				) as string;
			}
		}
		return icons;
	});

	const filterClass = (value: string) =>
		cn('btn btn-sm', selectedFilter === value ? 'bg-secondary-500/25' : '');
	const sortButtonClass = (value: SortBy) =>
		cn('btn', sortBy === value ? 'bg-secondary-500/25' : '');

	const NameSortIcon = $derived.by(() => {
		if (sortBy !== 'name') return ArrowDownAZ;
		return sortOrder === 'asc' ? ArrowDownAZ : ArrowDownZA;
	});
	const PaldeckSortIcon = $derived.by(() => {
		if (sortBy !== 'paldeck-index') return ArrowDownWideNarrow;
		return sortOrder === 'asc' ? ArrowDownWideNarrow : ArrowDownNarrowWide;
	});

	function toggleSort(newSortBy: SortBy) {
		if (sortBy === newSortBy) {
			if (sortOrder === 'desc') {
				sortBy = 'paldeck-index';
				sortOrder = 'asc';
			} else {
				sortOrder = 'desc';
			}
		} else {
			sortBy = newSortBy;
			sortOrder = 'asc';
		}
	}

	function getElementIcon(element: string): string {
		const el = elementsData.elements[element];
		if (!el) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${el.icon}.webp`) as string;
	}

	const allPals = $derived(Object.entries(palsData.pals).filter(([, pal]) => !pal.disabled));

	const filteredPals = $derived.by(() => {
		let result = allPals;

		if (selectedFilter !== 'All') {
			if (selectedFilter === 'alpha') {
				result = result.filter(([, pal]) => pal.is_boss || pal.is_tower_boss || pal.is_raid_boss);
			} else if (selectedFilter === 'human') {
				result = result.filter(([, pal]) => !pal.is_pal);
			} else {
				result = result.filter(([, pal]) =>
					pal.element_types.some((e) => e.toLowerCase() === selectedFilter.toLowerCase())
				);
			}
		}

		if (search) {
			const q = search.toLowerCase();
			result = result.filter(
				([key, pal]) =>
					pal.localized_name.toLowerCase().includes(q) || key.toLowerCase().includes(q)
			);
		}

		result = [...result].sort((a, b) => {
			let cmp = 0;
			switch (sortBy) {
				case 'name':
					cmp = a[1].localized_name.localeCompare(b[1].localized_name);
					break;
				case 'paldeck-index':
					cmp = byPaldeckIndex(a[1].pal_deck_index, b[1].pal_deck_index);
					break;
			}
			return sortOrder === 'asc' ? cmp : -cmp;
		});

		return result;
	});
</script>

<svelte:head>
	<title>Pals | Palworld Save Pal Wiki</title>
</svelte:head>

<div>
	<WikiGrid items={filteredPals}>
		{#snippet toolbar()}
			<div class="flex flex-wrap items-center gap-3">
				<div class="min-w-48 flex-1">
					<WikiSearch bind:value={search} />
				</div>
				<div class="flex items-center gap-1">
					<button
						type="button"
						class={sortButtonClass('name')}
						onclick={() => toggleSort('name')}
						title="Name"
					>
						<NameSortIcon class="h-4 w-4" />
					</button>
					<button
						type="button"
						class={sortButtonClass('paldeck-index')}
						onclick={() => toggleSort('paldeck-index')}
						title="Paldeck #"
					>
						<PaldeckSortIcon class="h-4 w-4" />
					</button>
				</div>
				<div class="flex flex-wrap items-center gap-1">
					<button type="button" class={filterClass('All')} onclick={() => (selectedFilter = 'All')}>
						<GalleryVerticalEnd class="h-4 w-4" />
					</button>
					{#each elementTypes as element (element)}
						<button
							type="button"
							class={filterClass(element)}
							onclick={() => (selectedFilter = element)}
						>
							<img src={elementIcons[element]} alt={element} class="h-5 w-5" />
						</button>
					{/each}
					<Tooltip label="Alpha / Boss">
						<button
							type="button"
							class={filterClass('alpha')}
							onclick={() => (selectedFilter = 'alpha')}
						>
							<img src={staticIcons.alphaIcon} alt="Alpha" class="h-5 w-5" />
						</button>
					</Tooltip>
					<Tooltip label="Human">
						<button
							type="button"
							class={filterClass('human')}
							onclick={() => (selectedFilter = 'human')}
						>
							<User class="h-4 w-4" />
						</button>
					</Tooltip>
				</div>
				<span class="text-surface-400 text-xs">{filteredPals.length}</span>
			</div>
		{/snippet}

		{#snippet children([key, pal])}
			<a
				href="/wiki/pals/{toSlug(key)}"
				class="border-surface-800 hover:border-primary-500/50 hover:bg-surface-700 flex flex-col items-center rounded-lg border p-3 text-center transition-colors"
			>
				<img
					src={assetLoader.loadPalImage(key, pal.is_pal)}
					alt={pal.localized_name}
					class="h-16 w-16 object-contain"
				/>
				<span class="mt-2 line-clamp-1 text-sm font-medium">{pal.localized_name}</span>
				{#if pal.pal_deck_index > 0}
					<span class="text-surface-400 text-xs">#{pal.pal_deck_index}</span>
				{/if}
				<div class="mt-1 flex items-center gap-1">
					{#each pal.element_types as element (element)}
						{@const icon = getElementIcon(element)}
						{#if icon}
							<img src={icon} alt={element} class="h-4 w-4 shrink-0" />
						{/if}
					{/each}
				</div>
			</a>
		{/snippet}
	</WikiGrid>
</div>
