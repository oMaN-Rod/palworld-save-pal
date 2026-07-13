<script lang="ts">
	import { passiveSkillsData } from '$lib/data';
	import type { PassiveSkill } from '$types';
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

	type SortBy = 'name' | 'rank';
	type SortOrder = 'asc' | 'desc';

	let search = $state('');
	let selectedFilter = $state('All');
	let sortBy: SortBy = $state('name');
	let sortOrder: SortOrder = $state('asc');
	let filterExpand = $state(['']);

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

	// Many BossDefeatReward_<Pal> keys (one per boss, for save-file
	// compatibility) share an identical generic name/rank/effect set, e.g.
	// 16 separate keys all called "Dark Damage Enhancement (Small)". Collapse
	// those to one representative row here; skill-select modals elsewhere
	// keep every key so a save's specific boss-reward passive stays pickable.
	function isBetterSkillEntry(
		[aKey]: [string, PassiveSkill],
		[bKey]: [string, PassiveSkill]
	): boolean {
		if (aKey.length !== bKey.length) return aKey.length < bKey.length;
		return aKey < bKey;
	}

	const allSkills = $derived.by(() => {
		const entries = Object.entries(passiveSkillsData.passiveSkills).filter(
			([, skill]) => !skill.details.disabled
		);
		const best = new Map<string, [string, PassiveSkill]>();
		for (const entry of entries) {
			const [, skill] = entry;
			const groupKey = `${skill.localized_name}|${skill.details.rank}|${JSON.stringify(skill.details.effects)}`;
			const prev = best.get(groupKey);
			if (!prev || isBetterSkillEntry(entry, prev)) best.set(groupKey, entry);
		}
		return [...best.values()];
	});

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
</script>

<div class="flex h-full flex-col gap-4">
	<span class="text-surface-400 mb-2 w-full text-end text-xs">{filteredSkills.length}</span>

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
							<div class="mt-1 grid grid-cols-2 gap-1">
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
									class={sortButtonClass('rank')}
									onclick={() => toggleSort('rank')}
									title="Rank"
								>
									<RankSortIcon class="h-4 w-4" />
								</button>
							</div>
						</div>
						<div>
							<legend class="text-surface-400 text-xs font-bold">Rank</legend>
							<div class="mt-1 grid grid-cols-5 gap-1">
								<button
									type="button"
									class={filterClass('All')}
									onclick={() => (selectedFilter = 'All')}
								>
									<GalleryVerticalEnd class="h-4 w-4" />
								</button>
								{#each [1, 2, 3, 4] as rank (rank)}
									{@const rankIcon = assetLoader.loadImage(
										`${ASSET_DATA_PATH}/img/rank_${rank}.webp`
									)}
									{@const filterStyle = skillFilter(rank)}
									<button
										type="button"
										class={filterClass(String(rank))}
										onclick={() => (selectedFilter = String(rank))}
									>
										<img
											src={rankIcon}
											alt="Rank {rank}"
											class="h-4 w-4"
											style="filter: {filterStyle};"
										/>
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
						<th>Rank</th>
						<th>Description</th>
						<th>Effects</th>
					</tr>
				</thead>
				<tbody class="[&>tr]:hover:preset-tonal-primary">
					{#each filteredSkills as [key, skill] (key)}
						{@const iterIcon = assetLoader.loadImage(
							`${ASSET_DATA_PATH}/img/rank_${skill.details.rank}.webp`
						)}
						{@const filterStyle = skillFilter(skill.details.rank)}
						<tr>
							<td class="font-medium">{skill.localized_name}</td>
							<td class="text-surface-400 font-mono text-xs">{key}</td>
							<td>
								<img
									src={iterIcon}
									alt="Rank {skill.details.rank}"
									class="h-4 w-4"
									style="filter: {filterStyle};"
								/>
							</td>
							<td class="text-surface-300 text-sm">{skill.description}</td>
							<td>
								{#if skill.details.effects.length > 0}
									<div class="flex flex-wrap gap-1">
										{#each skill.details.effects as effect, i (i)}
											<span
												class="bg-surface-900 inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-xs"
											>
												<span class="text-surface-300">{effect.type}</span>
												<span
													class="font-semibold {effect.value > 0
														? 'text-green-400'
														: 'text-red-400'}"
												>
													{effect.value > 0 ? '+' : ''}{effect.value}%
												</span>
											</span>
										{/each}
									</div>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</div>
</div>
