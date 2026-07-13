<script lang="ts">
	import { palsData, elementsData, activeSkillsData } from '$lib/data';
	import { WikiSearch } from '$components/docs';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import {
		assetLoader,
		getWorkSuitabilityFormattedName,
		suitabilityImageMap,
		wazaIdFromStr
	} from '$utils';
	import { c } from '$lib/utils/commonTranslations';
	import type { PalData, WorkSuitability } from '$types';
	import { staticIcons } from '$types/icons';
	import { Tooltip } from '$components/ui';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { cn } from '$theme';
	import {
		SlidersHorizontal,
		ArrowDownAZ,
		ArrowDownZA,
		ArrowDownWideNarrow,
		ArrowDownNarrowWide,
		GalleryVerticalEnd,
		User
	} from 'lucide-svelte';
	import type { ValueChangeDetails } from '@zag-js/accordion';

	let search = $state('');
	let selectedKey = $state<string | null>(null);
	let selectedFilter = $state('All');
	let sortBy: SortBy = $state('paldeck-index');
	let sortOrder: SortOrder = $state('asc');
	let filterExpand = $state(['']);

	type SortBy = 'name' | 'paldeck-index';
	type SortOrder = 'asc' | 'desc';

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

	function getWorkSuitabilities(pal: PalData): [string, string, number][] {
		return Object.entries(pal.work_suitability)
			.filter(([, val]) => val > 0)
			.map(([key, val]) => [key, getWorkSuitabilityFormattedName(key as WorkSuitability), val]);
	}

	// Variants (raid body parts, gym phases, duplicate generic NPCs) share a
	// display name; the wiki shows one representative entry per name/elements.
	function isBetterPalEntry(
		[aKey, aPal]: [string, PalData],
		[bKey, bPal]: [string, PalData]
	): boolean {
		const aDeck = (aPal.pal_deck_index ?? -1) >= 0 ? 0 : 1;
		const bDeck = (bPal.pal_deck_index ?? -1) >= 0 ? 0 : 1;
		if (aDeck !== bDeck) return aDeck < bDeck;
		if (aKey.length !== bKey.length) return aKey.length < bKey.length;
		return aKey < bKey;
	}

	const allPals = $derived.by(() => {
		const entries = Object.entries(palsData.pals).filter(([, pal]) => !pal.disabled);
		const best = new Map<string, [string, PalData]>();
		for (const entry of entries) {
			const [, pal] = entry;
			const groupKey = `${pal.localized_name}|${pal.is_pal}|${[...(pal.element_types ?? [])].sort().join(',')}`;
			const prev = best.get(groupKey);
			if (!prev || isBetterPalEntry(entry, prev)) best.set(groupKey, entry);
		}
		return [...best.values()];
	});

	const filteredPals = $derived.by(() => {
		let result = allPals;

		if (selectedFilter !== 'All') {
			if (selectedFilter === 'alpha') {
				result = result.filter(([, pal]) => pal.is_boss || pal.is_tower_boss || pal.is_raid_boss);
			} else if (selectedFilter === 'human') {
				result = result.filter(([, pal]) => !pal.is_pal);
			} else {
				// Element filter
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
					cmp = a[1].pal_deck_index - b[1].pal_deck_index;
					break;
			}
			return sortOrder === 'asc' ? cmp : -cmp;
		});

		return result;
	});

	const selectedPal = $derived(selectedKey ? palsData.pals[selectedKey] : null);
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<span class="text-surface-400 text-xs w-full text-end mb-2">{filteredPals.length}</span>
		<div class="mb-3">
			<Accordion
				value={filterExpand}
				onValueChange={(e: ValueChangeDetails) => (filterExpand = e.value)}
				collapsible
			>
				<Accordion.Item
					value="filter"
					base="rounded-sm bg-surface-900"
					controlHover="hover:bg-secondary-500/25"
				>
					{#snippet lead()}<SlidersHorizontal class="h-4 w-4" />{/snippet}
					{#snippet control()}<span class="text-sm font-bold">Filter & Sort</span>{/snippet}
					{#snippet panel()}
						<div class="mb-2">
							<legend class="text-xs font-bold text-surface-400">Sort</legend>
							<div class="mt-1 grid grid-cols-2 gap-1">
								<button type="button" class={sortButtonClass('name')} onclick={() => toggleSort('name')} title="Name">
									<NameSortIcon class="h-4 w-4" />
								</button>
								<button type="button" class={sortButtonClass('paldeck-index')} onclick={() => toggleSort('paldeck-index')} title="Paldeck #">
									<PaldeckSortIcon class="h-4 w-4" />
								</button>
							</div>
						</div>
						<div>
							<legend class="text-xs font-bold text-surface-400">Element & Type</legend>
							<div class="mt-1 grid grid-cols-4 gap-1">
								<button type="button" class={filterClass('All')} onclick={() => (selectedFilter = 'All')}>
									<GalleryVerticalEnd class="h-4 w-4" />
								</button>
								{#each elementTypes as element}
									<button type="button" class={filterClass(element)} onclick={() => (selectedFilter = element)}>
										<img src={elementIcons[element]} alt={element} class="h-5 w-5" />
									</button>
								{/each}
								<Tooltip label="Alpha / Boss">
									<button type="button" class={filterClass('alpha')} onclick={() => (selectedFilter = 'alpha')}>
										<img src={staticIcons.alphaIcon} alt="Alpha" class="h-5 w-5" />
									</button>
								</Tooltip>
								<Tooltip label="Human">
									<button type="button" class={filterClass('human')} onclick={() => (selectedFilter = 'human')}>
										<User class="h-4 w-4" />
									</button>
								</Tooltip>
							</div>
						</div>
					{/snippet}
				</Accordion.Item>
			</Accordion>
		</div>
		<div class="mb-3">
			<WikiSearch bind:value={search} />
		</div>
		<div class="flex-1 overflow-y-auto">
			{#each filteredPals as [key, pal]}
				<button
					class="flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedKey ===
					key
						? 'bg-surface-700 text-surface-50'
						: 'text-surface-300 hover:bg-surface-800'}"
					onclick={() => (selectedKey = key)}
				>
					{#if pal.pal_deck_index > 0}
						<span class="text-surface-400 text-xs">#{pal.pal_deck_index}</span>
					{/if}
					<span class="ml-2 flex-1 truncate text-left font-medium">{pal.localized_name}</span>
					<div class="flex items-center gap-1">
						{#each pal.element_types as element}
							{@const icon = getElementIcon(element)}
							{#if icon}
								<img src={icon} alt={element} class="h-4 w-4 shrink-0" />
							{/if}
						{/each}
					</div>
				</button>
			{/each}
		</div>
	</div>

	<div class="border-surface-800 flex-1 overflow-y-auto rounded-lg border p-5">
		{#if selectedPal && selectedKey}
			{@const workSuits = getWorkSuitabilities(selectedPal)}
			<div class="flex items-center gap-3">
				<h2 class="text-2xl font-bold">{selectedPal.localized_name}</h2>
				{#each selectedPal.element_types as element}
					{@const icon = getElementIcon(element)}
					{#if icon}
						<img src={icon} alt={element} class="h-6 w-6" />
					{/if}
				{/each}
				<span class="text-surface-400 text-sm">#{selectedPal.pal_deck_index}</span>
				<span class="text-surface-400 text-sm">{selectedKey}</span>
			</div>

			<p class="text-surface-300 mt-3">{selectedPal.description}</p>

			<div class="mt-5 grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)] gap-6">
				<div class="flex items-center justify-center">
					<img
						src={assetLoader.loadPalImage(selectedKey, selectedPal?.is_pal || false)}
						alt={`${selectedPal?.localized_name} icon`}
						class="max-h-87.5 max-w-full 2xl:max-h-150"
					/>
				</div>
				<div>
					<div class="mt-5 grid grid-cols-3 gap-4">
						<div class="bg-surface-900 rounded-md p-3">
							<div class="flex gap-2">
								<img src={staticIcons.hpIcon} alt="HP icon" class="h-4 w-4" />
								<span class="text-surface-500 text-xs">HP</span>
							</div>
							<p class="text-lg font-semibold">{selectedPal.scaling.hp}</p>
						</div>
						<div class="bg-surface-900 rounded-md p-3">
							<div class="flex gap-2">
								<img src={staticIcons.attackIcon} alt="Attack icon" class="h-4 w-4" />
								<span class="text-surface-500 text-xs">Attack</span>
							</div>
							<p class="text-lg font-semibold">{selectedPal.scaling.attack}</p>
						</div>
						<div class="bg-surface-900 rounded-md p-3">
							<div class="flex gap-2">
								<img src={staticIcons.defenseIcon} alt="Defense icon" class="h-4 w-4" />
								<span class="text-surface-500 text-xs">Defense</span>
							</div>
							<p class="text-lg font-semibold">{selectedPal.scaling.defense}</p>
						</div>
					</div>

					<div class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-4">
						<div>
							<span class="text-surface-500 text-xs">Size</span>
							<p class="text-sm">{selectedPal.size}</p>
						</div>
						<div>
							<span class="text-surface-500 text-xs">Rarity</span>
							<p class="text-sm">{selectedPal.rarity}</p>
						</div>
						<div>
							<span class="text-surface-500 text-xs">Food</span>
							<p class="text-sm">{selectedPal.food_amount}</p>
						</div>
						<div>
							<span class="text-surface-500 text-xs">Stamina</span>
							<p class="text-sm">{selectedPal.stamina}</p>
						</div>
						<div>
							<span class="text-surface-500 text-xs">Walk Speed</span>
							<p class="text-sm">{selectedPal.walk_speed}</p>
						</div>
						<div>
							<span class="text-surface-500 text-xs">Run Speed</span>
							<p class="text-sm">{selectedPal.run_speed}</p>
						</div>
						<div>
							<span class="text-surface-500 text-xs">Ride Sprint</span>
							<p class="text-sm">{selectedPal.ride_sprint_speed}</p>
						</div>
						<div>
							<span class="text-surface-500 text-xs">Capture Rate</span>
							<p class="text-sm">{selectedPal.capture_rate_correct}</p>
						</div>
					</div>

					{#if workSuits.length > 0}
						<div class="mt-5">
							<h3 class="text-surface-400 mb-2 text-sm font-semibold">Work Suitability</h3>
							<div class="flex flex-wrap gap-2">
								{#each workSuits as [key, type, level]}
									{@const iconPath = assetLoader.loadImage(
										`${ASSET_DATA_PATH}/img/${suitabilityImageMap[key as WorkSuitability]}.webp`
									)}
									<div class="bg-surface-900 flex items-center gap-2 rounded-md px-3 py-1 text-sm">
										<img src={iconPath} alt="{type} icon" class="h-4 w-4 2xl:h-6 2xl:w-6" />
										<div>
											{type} <span class="text-surface-400 font-semibold">Lv.{level}</span>
										</div>
									</div>
								{/each}
							</div>
						</div>
					{/if}

					{#if selectedPal.passive_skills.length > 0}
						<div class="mt-5">
							<h3 class="text-surface-400 mb-2 text-sm font-semibold">Partner Skills</h3>
							<div class="flex flex-wrap gap-2">
								{#each selectedPal.passive_skills as skill}
									<span class="bg-surface-900 rounded-md px-3 py-1 text-sm">{skill}</span>
								{/each}
							</div>
						</div>
					{/if}

					{#if selectedPal.skill_set && Object.keys(selectedPal.skill_set).length > 0}
						<div class="mt-5">
							<h3 class="text-surface-400 mb-2 text-sm font-semibold">Skill Set</h3>
							<div class="flex flex-wrap gap-2">
								{#each Object.entries(selectedPal.skill_set) as [skill, level]}
									{@const [_, skillId] = wazaIdFromStr(`EPalWazaID::${skill}`)}
									{@const skillData = activeSkillsData.getByKey(skillId)}
									{@const skillElement = skillData?.details?.element}
									{@const skillElementIcon = skillElement ? getElementIcon(skillElement) : ''}
									<div class="bg-surface-900 flex items-center gap-2 rounded-md px-3 py-1 text-sm">
										{#if skillElementIcon}
											<img src={skillElementIcon} alt={skillElement} class="h-4 w-4" />
										{/if}
										<span>{skillData?.localized_name || skill}</span>
										<span class="text-surface-400">Lv.{level}</span>
									</div>
								{/each}
							</div>
						</div>
					{/if}
				</div>
			</div>
		{:else}
			<div class="text-surface-500 flex h-full items-center justify-center">
				<p>Select a Pal to view details</p>
			</div>
		{/if}
	</div>
</div>
