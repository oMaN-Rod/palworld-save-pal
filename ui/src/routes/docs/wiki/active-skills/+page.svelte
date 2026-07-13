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

	type SortBy = 'name' | 'power' | 'cooldown';
	type SortOrder = 'asc' | 'desc';

	let search = $state('');
	let selectedFilter = $state('All');
	let sortBy: SortBy = $state('name');
	let sortOrder: SortOrder = $state('asc');
	let filterExpand = $state(['']);

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
</script>

<div class="flex h-full flex-col gap-4">

	<span class="text-surface-400 text-xs w-full text-end mb-2">{filteredSkills.length}</span>

	<div class="flex flex-col gap-3 md:flex-row md:items-start">
		<div class="md:w-72">
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
							<legend class="text-surface-400 text-xs font-bold">Sort</legend>
							<div class="mt-1 grid grid-cols-3 gap-1">
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
									class={sortButtonClass('power')}
									onclick={() => toggleSort('power')}
									title="Power"
								>
									<PowerSortIcon class="h-4 w-4" />
								</button>
								<button
									type="button"
									class={sortButtonClass('cooldown')}
									onclick={() => toggleSort('cooldown')}
									title="Cooldown"
								>
									<CooldownSortIcon class="h-4 w-4" />
								</button>
							</div>
						</div>
						<div>
							<legend class="text-surface-400 text-xs font-bold">Element</legend>
							<div class="mt-1 grid grid-cols-4 gap-1">
								<button
									type="button"
									class={filterClass('All')}
									onclick={() => (selectedFilter = 'All')}
								>
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
							</div>
						</div>
					{/snippet}
				</Accordion.Item>
			</Accordion>
		</div>
		<div class="flex-1">
			<WikiSearch bind:value={search} />
		</div>
	</div>

	<div class="min-h-0 flex-1">
		<div class="table-wrap h-full overflow-y-auto">
			<table class="table caption-bottom">
				<thead class="bg-surface-950 sticky top-0 z-10">
					<tr>
						<th>Name</th>
						<th>Code Name</th>
						<th>Element</th>
						<th class="text-right">CT</th>
						<th class="text-right">Power</th>
						<th>Description</th>
					</tr>
				</thead>
				<tbody class="[&>tr]:hover:preset-tonal-primary">
					{#each filteredSkills as [key, skill] (key)}
						<tr>
							<td>
								<div class="flex items-center gap-2">
									<span class="font-medium">{skill.localized_name}</span>
								</div>
							</td>
							<td class="text-surface-400 font-mono text-xs">{key}</td>
							<td>
								{#if getElementIcon(skill.details.element)}
									<img
										src={getElementIcon(skill.details.element)}
										alt=""
										class="h-4 w-4 shrink-0"
									/>
								{/if}
							</td>
							<td class="text-right">{skill.details.cool_time}s</td>
							<td class="text-right">{skill.details.power}</td>
							<td class="text-surface-300 text-sm">{skill.description}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</div>
</div>
