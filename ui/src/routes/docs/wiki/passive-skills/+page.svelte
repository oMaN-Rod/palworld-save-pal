<script lang="ts">
	import { passiveSkillsData } from '$lib/data';
	import { WikiSearch } from '$components/docs';
	import * as m from '$i18n/messages';
	import { assetLoader, skillFilter } from '$utils';
	import { ASSET_DATA_PATH } from '$lib/constants';
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

	type SortBy = 'name' | 'rank';
	type SortOrder = 'asc' | 'desc';

	const filterClass = (value: string) =>
		cn('btn btn-sm px-2 py-1 rounded', selectedFilter === value ? 'bg-secondary-500/25' : '');
	const sortButtonClass = (value: SortBy) =>
		cn('btn', sortBy === value ? 'bg-secondary-500/25' : '');

	const NameSortIcon = $derived.by(() => {
		if (sortBy !== 'name') return ArrowDownAZ;
		return sortOrder === 'asc' ? ArrowDownAZ : ArrowDownZA;
	});
	const RankSortIcon = $derived.by(() => {
		if (sortBy !== 'rank') return ArrowDown01;
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

	const allSkills = $derived(
		Object.entries(passiveSkillsData.passiveSkills)
	);

	const filteredSkills = $derived.by(() => {
		let result = allSkills;

		if (selectedFilter !== 'All') {
			const rank = parseInt(selectedFilter);
			result = result.filter(([, skill]) => skill.details.rank === rank);
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
				case 'rank':
					cmp = a[1].details.rank - b[1].details.rank;
					break;
			}
			return sortOrder === 'asc' ? cmp : -cmp;
		});

		return result;
	});

	const selectedSkill = $derived(selectedKey ? passiveSkillsData.passiveSkills[selectedKey] : null);
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="mb-3 flex items-center justify-between">
			<h1 class="text-lg font-bold">{m.passive_skill({ count: 2 })}</h1>
			<span class="text-surface-400 text-xs">{filteredSkills.length}</span>
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
							<div class="mt-1 grid grid-cols-2 gap-1">
								<button type="button" class={sortButtonClass('name')} onclick={() => toggleSort('name')} title="Name">
									<NameSortIcon class="h-4 w-4" />
								</button>
								<button type="button" class={sortButtonClass('rank')} onclick={() => toggleSort('rank')} title="Rank">
									<RankSortIcon class="h-4 w-4" />
								</button>
							</div>
						</div>
						<div>
							<legend class="text-xs font-bold text-surface-400">Rank</legend>
							<div class="mt-1 grid grid-cols-5 gap-1">
								<button type="button" class={filterClass('All')} onclick={() => (selectedFilter = 'All')}>
									<GalleryVerticalEnd class="h-4 w-4" />
								</button>
								{#each [1, 2, 3, 4] as rank}
									{@const rankIcon = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/rank_${rank}.webp`)}
									{@const filterStyle = skillFilter(rank)}
									<button type="button" class={filterClass(String(rank))} onclick={() => (selectedFilter = String(rank))}>
										<img src={rankIcon} alt="Rank {rank}" class="h-4 w-4" style="filter: {filterStyle};" />
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
				{@const iterIcon = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/rank_${skill.details.rank}.webp`
				)}
				{@const filterStyle = skillFilter(skill.details.rank)}
				<button
					class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedKey ===
					key
						? 'bg-surface-700 text-surface-50'
						: 'text-surface-300 hover:bg-surface-800'}"
					onclick={() => (selectedKey = key)}
				>
					<span class="truncate font-medium">{skill.localized_name}</span>
					<div class="ml-auto text-xs font-medium">
						<img
							src={iterIcon}
							alt="Skill rank icon"
							class="h-4 w-4"
							style="filter: {filterStyle};"
						/>
					</div>
				</button>
			{/each}
		</div>
	</div>

	<div class="border-surface-800 flex-1 overflow-y-auto rounded-lg border p-5">
		{#if selectedSkill && selectedKey}
			{@const iterIcon = assetLoader.loadImage(
				`${ASSET_DATA_PATH}/img/rank_${selectedSkill.details.rank}.webp`
			)}
			{@const filterStyle = skillFilter(selectedSkill.details.rank)}
			<div class="flex items-center gap-2">
				<div class="text-xs font-medium">
					<img
						src={iterIcon}
						alt="Skill rank icon"
						class="h-5 w-5"
						style="filter: {filterStyle};"
					/>
				</div>
				<h2 class="text-2xl font-bold">{selectedSkill.localized_name}</h2>
			</div>

			<p class="text-surface-300 mt-3">{selectedSkill.description}</p>

			{#if selectedSkill.details.effects.length > 0}
				<div class="mt-5">
					<h3 class="text-surface-400 mb-2 text-sm font-semibold">Effects</h3>
					<div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
						{#each selectedSkill.details.effects as effect}
							<div class="bg-surface-900 flex items-center justify-between rounded-md px-3 py-2">
								<span class="text-sm">{effect.type}</span>
								<span class="font-semibold {effect.value > 0 ? 'text-green-400' : 'text-red-400'}">
									{effect.value > 0 ? '+' : ''}{effect.value}%
								</span>
							</div>
						{/each}
					</div>
				</div>
			{/if}
		{:else}
			<div class="text-surface-500 flex h-full items-center justify-center">
				<p>Select a skill to view details</p>
			</div>
		{/if}
	</div>
</div>
