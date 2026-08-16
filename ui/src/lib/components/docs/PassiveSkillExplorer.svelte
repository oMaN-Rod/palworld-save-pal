<script lang="ts">
	import { passiveSkillsData } from '$lib/data';
	import { groupPassiveFamilies, type PassiveFamily, type PassiveMember } from '$lib/utils/passiveFamilies';
	import { isDisabledRecord } from '$lib/utils/wikiSlug';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { skillBorderClass, skillFilter } from '$utils/colors';
	import WikiSearch from './WikiSearch.svelte';

	let search = $state('');
	let selectedFamilyKey = $state<string | null>(null);
	let selectedMemberKey = $state<string | null>(null);

	const allEntries = $derived(
		Object.entries(passiveSkillsData.passiveSkills).filter(([, record]) =>
			!isDisabledRecord(record)
		)
	);

	const families = $derived(groupPassiveFamilies(allEntries));

	const filteredFamilies = $derived.by(() => {
		if (!search.trim()) return families;
		const q = search.toLowerCase();
		return families.filter((f) => f.displayName.toLowerCase().includes(q));
	});

	const selectedFamily = $derived(
		families.find((f) => f.key === selectedFamilyKey) ?? filteredFamilies[0] ?? null
	);

	const activeMember = $derived.by(() => {
		if (!selectedFamily) return null;
		if (selectedMemberKey) {
			const found = selectedFamily.members.find((m) => m.key === selectedMemberKey);
			if (found) return found;
		}
		return selectedFamily.members[0];
	});

	const familyRankTabs = $derived(
		selectedFamily && selectedFamily.ranks.length > 1
			? selectedFamily.members.filter(
					(m, i, arr) =>
						i === arr.findIndex((other) => other.skill.details.rank === m.skill.details.rank)
				)
			: []
	);

	const sameRankMembers = $derived.by((): PassiveMember[] => {
		if (!selectedFamily || !activeMember) return [];
		const rank = activeMember.skill.details.rank;
		return selectedFamily.members.filter((m) => m.skill.details.rank === rank);
	});

	function rankIcon(rank: number): string {
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/rank_${rank}.webp`) as string;
	}

	function selectFamily(family: PassiveFamily) {
		selectedFamilyKey = family.key;
		selectedMemberKey = null;
	}

	function effectTypeLabel(type: string): string {
		return type.replace(/^ElementResist_/, '').replace(/_/g, ' ');
	}
</script>

<div class="flex flex-col gap-4">
	<WikiSearch bind:value={search} />

	<div class="grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,300px)_minmax(0,1fr)]">
		<!-- Family list -->
		<div class="max-h-[70vh] overflow-y-auto pr-1">
			<ul class="flex flex-col gap-1">
				{#each filteredFamilies as family (family.key)}
					<li>
						<button
							type="button"
							class="hover:bg-surface-800 flex w-full items-center gap-2 rounded-md border-l-2 px-2 py-1.5 text-left text-sm transition-colors {skillBorderClass(family.primaryRank)} {selectedFamily?.key === family.key ? 'bg-surface-800' : ''}"
							onclick={() => selectFamily(family)}
						>
							<img
								src={rankIcon(family.primaryRank)}
								alt=""
								class="h-5 w-5 shrink-0 object-contain"
								style={(() => {
									const f = skillFilter(family.primaryRank);
									return f ? `filter: ${f};` : undefined;
								})()}
							/>
							<span class="truncate">{family.displayName}</span>
							{#if family.members.length > 1}
								<span class="text-surface-500 ml-auto shrink-0 text-xs">{family.members.length}</span>
							{/if}
						</button>
					</li>
				{/each}
			</ul>
		</div>

		<!-- Detail panel -->
		{#if selectedFamily}
			<div class="border-surface-800 rounded-lg border p-4">
				<div class="mb-3 flex items-center gap-3">
					<img
						src={rankIcon(selectedFamily.primaryRank)}
						alt=""
						class="h-10 w-10 object-contain"
						style={skillFilter(selectedFamily.primaryRank) ? `filter: ${skillFilter(selectedFamily.primaryRank)};` : undefined}
					/>
					<div class="flex flex-col">
						<h2 class="text-lg font-semibold">{selectedFamily.displayName}</h2>
						<span class="text-surface-400 text-xs">{selectedFamily.members.length} variant{selectedFamily.members.length === 1 ? '' : 's'}</span>
					</div>
				</div>

				<!-- Rank tabs: only when more than one distinct rank -->
				{#if familyRankTabs.length > 1}
					<div class="mb-4 flex flex-wrap gap-1">
						{#each familyRankTabs as tab (tab.skill.details.rank)}
							{@const isActive = activeMember?.skill.details.rank === tab.skill.details.rank}
							<button
								type="button"
								class="border-l-2 px-2.5 py-1 text-sm transition-colors {skillBorderClass(tab.skill.details.rank)} {isActive ? 'bg-surface-700 text-surface-50' : 'text-surface-400 hover:bg-surface-800'}"
								onclick={() => (selectedMemberKey = tab.key)}
							>
								Rank {tab.skill.details.rank}
							</button>
						{/each}
					</div>
				{/if}

				{#if activeMember}
					{@const member = activeMember}
					<div class="mb-3 flex flex-wrap items-center gap-2">
						<span class="rounded bg-surface-800 px-2 py-0.5 font-mono text-xs">Rank {member.skill.details.rank}</span>
						<span class="text-surface-400 font-mono text-xs">{member.key}</span>
					</div>

					{#if member.skill.description}
						<p class="text-surface-300 mb-3 text-sm">{member.skill.description}</p>
					{/if}

					{#if member.skill.details.effects.length > 0}
						<div class="mb-3">
							<h3 class="text-surface-400 mb-1 text-xs font-semibold uppercase tracking-wide">Effects</h3>
							<table class="w-full text-sm">
								<thead>
									<tr class="text-surface-400 border-b border-surface-800 text-left text-xs">
										<th class="py-1 pr-2 font-medium">Type</th>
										<th class="py-1 pr-2 font-medium">Value</th>
										<th class="py-1 font-medium">Target</th>
									</tr>
								</thead>
								<tbody>
									{#each member.skill.details.effects as effect (effect.type)}
										<tr class="border-b border-surface-800/50">
											<td class="py-1 pr-2">{effectTypeLabel(effect.type)}</td>
											<td class="py-1 pr-2 font-mono">{effect.value}</td>
											<td class="text-surface-400 py-1 text-xs">{effect.target}</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					{/if}

					{#if sameRankMembers.length > 1}
						<div>
							<h3 class="text-surface-400 mb-1 text-xs font-semibold uppercase tracking-wide">Same rank</h3>
							<ul class="flex flex-col gap-1">
								{#each sameRankMembers as sibling (sibling.key)}
									<li>
										<button
											type="button"
											class="hover:bg-surface-800 w-full rounded px-2 py-1 text-left font-mono text-xs {sibling.key === member.key ? 'text-surface-50' : 'text-surface-400'}"
											onclick={() => (selectedMemberKey = sibling.key)}
										>
											{sibling.key}
										</button>
									</li>
								{/each}
							</ul>
						</div>
					{/if}
				{/if}
			</div>
		{:else}
			<div class="text-surface-400 flex items-center justify-center py-12 text-sm">
				<p>No passive skills found.</p>
			</div>
		{/if}
	</div>
</div>
