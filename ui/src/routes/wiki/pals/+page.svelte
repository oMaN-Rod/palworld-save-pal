<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Seo, breadcrumbSchema } from '$lib/components/seo';
	import { palsData, elementsData } from '$lib/data';
	import { WikiGrid, WikiSearch, WikiCard, WikiViewToggle } from '$components/docs';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { toSlug } from '$lib/utils/wikiSlug';
	import { byPaldeckIndex } from '$lib/utils/wikiDescriptors';
	import { classifyPalCategory, type PalCategory } from '$lib/utils/palFilters';
	import { wikiPrefs } from '$lib/utils/wikiPrefs.svelte';
	import { cn } from '$theme';

	type SortBy = 'name' | 'paldeck-index';
	type SortOrder = 'asc' | 'desc';

	const CATEGORIES: { id: PalCategory; label: string }[] = [
		{ id: 'normal', label: 'Normal' },
		{ id: 'quest', label: 'Quest' },
		{ id: 'boss', label: 'Boss' },
		{ id: 'special', label: 'Special' },
		{ id: 'other', label: 'Other' }
	];

	let search = $state('');
	// Multi-select category set; defaults to normal-only so the common case is uncluttered.
	let selectedCategories = $state<Set<PalCategory>>(new Set<PalCategory>(['normal']));
	let selectedElement = $state<string | null>(null);
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

	const sortButtonClass = (value: SortBy) =>
		cn('btn', sortBy === value ? 'bg-secondary-500/25' : '');

	const NameSortIcon = $derived.by(() => {
		if (sortBy !== 'name') return 'tabler:sort-ascending-letters';
		return sortOrder === 'asc' ? 'tabler:sort-ascending-letters' : 'tabler:sort-descending-letters';
	});
	const PaldeckSortIcon = $derived.by(() => {
		if (sortBy !== 'paldeck-index') return 'tabler:arrows-sort';
		return sortOrder === 'asc' ? 'tabler:arrows-sort' : 'tabler:arrows-sort';
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

	function toggleCategory(id: PalCategory) {
		const next = new Set(selectedCategories);
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
		}
		// Never let the set go fully empty — re-add the toggled one.
		selectedCategories = next.size > 0 ? next : new Set<PalCategory>([id]);
	}

	function toggleElement(element: string) {
		selectedElement = selectedElement === element ? null : element;
	}

	const categoryChipClass = (id: PalCategory) =>
		cn(
			'btn btn-sm gap-1',
			selectedCategories.has(id) ? 'bg-secondary-500/25 text-surface-50' : 'text-surface-400'
		);
	const elementChipClass = (element: string) =>
		cn('btn btn-sm', selectedElement === element ? 'bg-secondary-500/25' : '');

	function getElementIcon(element: string): string {
		const el = elementsData.elements[element];
		if (!el) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${el.icon}.webp`) as string;
	}

	const allPals = $derived(Object.entries(palsData.pals).filter(([, pal]) => !pal.disabled));

	const filteredPals = $derived.by(() => {
		let result = allPals.filter(([key, pal]) => {
			const category = classifyPalCategory(key, pal);
			if (!selectedCategories.has(category)) return false;
			if (selectedElement && !pal.element_types.some((e) => e === selectedElement)) return false;
			return true;
		});

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

<Seo
	pathname="/wiki/pals"
	title="All Palworld Pals - Stats, Elements and Skills"
	description="Every Pal in Palworld with elements, work suitability and base stats, searchable and sortable."
	structuredData={breadcrumbSchema([
		{ name: 'Wiki', path: '/wiki' },
		{ name: 'Pals', path: '/wiki/pals' }
	])}
/>

<div>
	<WikiGrid items={filteredPals}>
		{#snippet toolbar()}
			<div class="flex flex-col gap-3">
				<div class="flex flex-wrap items-center gap-2">
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
							<Icon icon={NameSortIcon} class="h-4 w-4" />
						</button>
						<button
							type="button"
							class={sortButtonClass('paldeck-index')}
							onclick={() => toggleSort('paldeck-index')}
							title="Paldeck #"
						>
							<Icon icon={PaldeckSortIcon} class="h-4 w-4" />
						</button>
					</div>
					<WikiViewToggle />
					<span class="text-surface-400 text-xs">{filteredPals.length}</span>
				</div>

				<div class="flex flex-wrap items-center gap-1">
					{#each CATEGORIES as { id, label } (id)}
						<button type="button" class={categoryChipClass(id)} onclick={() => toggleCategory(id)}>
							{#if selectedCategories.has(id)}
								<Icon icon="tabler:check" class="h-3.5 w-3.5" />
							{/if}
							{label}
						</button>
					{/each}
				</div>

				<div class="flex flex-wrap items-center gap-1">
					<button
						type="button"
						class={cn('btn btn-sm', selectedElement === null ? 'bg-secondary-500/25' : '')}
						onclick={() => (selectedElement = null)}
						title="All elements"
					>
						<Icon icon="tabler:layout-list" class="h-4 w-4" />
					</button>
					{#each elementTypes as element (element)}
						<button
							type="button"
							class={elementChipClass(element)}
							onclick={() => toggleElement(element)}
						>
							<img src={elementIcons[element]} alt={element} class="h-5 w-5" />
						</button>
					{/each}
				</div>
			</div>
		{/snippet}

		{#snippet children([key, pal])}
			<WikiCard
				href="/wiki/pals/{toSlug(key)}"
				name={pal.localized_name}
				variant={wikiPrefs.viewMode}
				gridIconClass="h-14 w-14"
				icon={(() => {
					const src = assetLoader.loadPalImage(key, pal.is_pal);
					return src ? { src } : null;
				})()}
			>
				{#snippet badges()}
					{#if pal.pal_deck_index > 0}
						<span class="text-surface-400 text-xs">#{pal.pal_deck_index}</span>
					{/if}
					{#each pal.element_types as element (element)}
						{@const icon = getElementIcon(element)}
						{#if icon}
							<img src={icon} alt={element} class="h-4 w-4 shrink-0" />
						{/if}
					{/each}
				{/snippet}
			</WikiCard>
		{/snippet}
	</WikiGrid>
</div>
