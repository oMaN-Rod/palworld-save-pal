<script lang="ts">
	import { activeSkillsData, elementsData } from '$lib/data';
	import { WikiSearch } from '$components/docs';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import * as m from '$i18n/messages';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { cn } from '$theme';
	import {
		SlidersHorizontal,
		ArrowDownAZ,
		ArrowDownZA,
		ArrowDown01,
		ArrowDown10,
		GalleryVerticalEnd
	} from 'lucide-svelte';
	import type { ValueChangeDetails } from '@zag-js/accordion';

	let search = $state('');
	let selectedKey = $state<string | null>(null);
	let selectedFilter = $state('All');
	let sortBy: SortBy = $state('name');
	let sortOrder: SortOrder = $state('asc');
	let filterExpand = $state(['']);

	type SortBy = 'name' | 'power' | 'cooldown';
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
	const PowerSortIcon = $derived.by(() => {
		if (sortBy !== 'power') return ArrowDown01;
		return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
	});
	const CooldownSortIcon = $derived.by(() => {
		if (sortBy !== 'cooldown') return ArrowDown01;
		return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
	});

	function toggleSort(newSortBy: SortBy) {
		if (sortBy === newSortBy) {
			if (sortOrder === 'desc') {
				sortBy = 'name';
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

	function elementColor(element: string): string {
		const el = elementsData.elements[element];
		return el?.color || '#888';
	}

	const allSkills = $derived(Object.entries(activeSkillsData.activeSkills));

	const filteredSkills = $derived.by(() => {
		let result = allSkills;

		if (selectedFilter !== 'All') {
			result = result.filter(([, skill]) => skill.details.element === selectedFilter);
		}

		if (search) {
			const q = search.toLowerCase();
			result = result.filter(
				([key, skill]) =>
					skill.localized_name.toLowerCase().includes(q) || key.toLowerCase().includes(q)
			);
		}

		result = [...result].sort((a, b) => {
			let cmp = 0;
			switch (sortBy) {
				case 'name':
					cmp = a[1].localized_name.localeCompare(b[1].localized_name);
					break;
				case 'power':
					cmp = a[1].details.power - b[1].details.power;
					break;
				case 'cooldown':
					cmp = a[1].details.cool_time - b[1].details.cool_time;
					break;
			}
			return sortOrder === 'asc' ? cmp : -cmp;
		});

		return result;
	});

	const selectedSkill = $derived(
		selectedKey ? activeSkillsData.activeSkills[selectedKey] : null
	);
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="mb-3 flex items-center justify-between">
			<h1 class="text-lg font-bold">{m.active_skill({ count: 2 })}</h1>
			<span class="text-xs text-surface-400">{filteredSkills.length}</span>
		</div>
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
							<div class="mt-1 grid grid-cols-3 gap-1">
								<button type="button" class={sortButtonClass('name')} onclick={() => toggleSort('name')} title="Name">
									<NameSortIcon class="h-4 w-4" />
								</button>
								<button type="button" class={sortButtonClass('power')} onclick={() => toggleSort('power')} title="Power">
									<PowerSortIcon class="h-4 w-4" />
								</button>
								<button type="button" class={sortButtonClass('cooldown')} onclick={() => toggleSort('cooldown')} title="Cooldown">
									<CooldownSortIcon class="h-4 w-4" />
								</button>
							</div>
						</div>
						<div>
							<legend class="text-xs font-bold text-surface-400">Element</legend>
							<div class="mt-1 grid grid-cols-4 gap-1">
								<button type="button" class={filterClass('All')} onclick={() => (selectedFilter = 'All')}>
									<GalleryVerticalEnd class="h-4 w-4" />
								</button>
								{#each elementTypes as element}
									<button type="button" class={filterClass(element)} onclick={() => (selectedFilter = element)}>
										<img src={elementIcons[element]} alt={element} class="h-5 w-5" />
									</button>
								{/each}
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
			{#each filteredSkills as [key, skill]}
				<button
					class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedKey === key
						? 'bg-surface-700 text-surface-50'
						: 'text-surface-300 hover:bg-surface-800'}"
					onclick={() => (selectedKey = key)}
				>
					<img src={getElementIcon(skill.details.element)} alt="" class="h-4 w-4 shrink-0" />
					<span class="truncate font-medium">{skill.localized_name}</span>
					<span class="ml-auto text-xs text-surface-500">{skill.details.power}</span>
				</button>
			{/each}
		</div>
	</div>

	<div class="flex-1 overflow-y-auto rounded-lg border border-surface-800 p-5">
		{#if selectedSkill && selectedKey}
			<div class="flex items-center gap-3">
				<img src={getElementIcon(selectedSkill.details.element)} alt="" class="h-8 w-8" />
				<div>
					<h2 class="text-2xl font-bold">{selectedSkill.localized_name}</h2>
					<span class="text-sm" style="color: {elementColor(selectedSkill.details.element)}">{selectedSkill.details.element}</span>
				</div>
			</div>

			<p class="text-surface-300 mt-3">{selectedSkill.description}</p>

			<div class="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Power</span>
					<p class="text-lg font-semibold">{selectedSkill.details.power}</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Cooldown</span>
					<p class="text-lg font-semibold">{selectedSkill.details.cool_time}s</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Type</span>
					<p class="text-sm">{selectedSkill.details.type}</p>
				</div>
				<div class="rounded-md bg-surface-900 p-3">
					<span class="text-xs text-surface-500">Range</span>
					<p class="text-sm">{selectedSkill.details.min_range} - {selectedSkill.details.max_range}</p>
				</div>
			</div>
		{:else}
			<div class="flex h-full items-center justify-center text-surface-500">
				<p>Select a skill to view details</p>
			</div>
		{/if}
	</div>
</div>
