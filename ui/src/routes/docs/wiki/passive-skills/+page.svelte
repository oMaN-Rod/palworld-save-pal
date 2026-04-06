<script lang="ts">
	import { passiveSkillsData } from '$lib/data';
	import { WikiSearch } from '$components/docs';
	import * as m from '$i18n/messages';
	import { assetLoader, skillFilter } from '$utils';
	import { ASSET_DATA_PATH } from '$lib/constants';

	let search = $state('');
	let selectedKey = $state<string | null>(null);

	const allSkills = $derived(
		Object.entries(passiveSkillsData.passiveSkills).sort((a, b) =>
			a[1].localized_name.localeCompare(b[1].localized_name)
		)
	);

	const filteredSkills = $derived(
		search
			? allSkills.filter(
					([key, skill]) =>
						skill.localized_name.toLowerCase().includes(search.toLowerCase()) ||
						key.toLowerCase().includes(search.toLowerCase())
				)
			: allSkills
	);

	const selectedSkill = $derived(selectedKey ? passiveSkillsData.passiveSkills[selectedKey] : null);

	function rankColor(rank: number): string {
		switch (rank) {
			case 1:
				return 'text-surface-300';
			case 2:
				return 'text-blue-400';
			case 3:
				return 'text-purple-400';
			case 4:
				return 'text-yellow-400';
			default:
				return 'text-surface-400';
		}
	}
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="mb-3 flex items-center justify-between">
			<h1 class="text-lg font-bold">{m.passive_skill({ count: 2 })}</h1>
			<span class="text-surface-400 text-xs">{filteredSkills.length}</span>
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
