<script lang="ts">
	import { activeSkillsData, elementsData } from '$lib/data';
	import { WikiSearch } from '$components/docs';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import * as m from '$i18n/messages';

	let search = $state('');
	let selectedKey = $state<string | null>(null);

	const allSkills = $derived(
		Object.entries(activeSkillsData.activeSkills).sort((a, b) =>
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

	const selectedSkill = $derived(
		selectedKey ? activeSkillsData.activeSkills[selectedKey] : null
	);

	function getElementIcon(element: string): string {
		const el = elementsData.elements[element];
		if (!el) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${el.icon}.webp`) as string;
	}

	function elementColor(element: string): string {
		const el = elementsData.elements[element];
		return el?.color || '#888';
	}
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="mb-3 flex items-center justify-between">
			<h1 class="text-lg font-bold">{m.active_skill({ count: 2 })}</h1>
			<span class="text-xs text-surface-400">{filteredSkills.length}</span>
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

	<div class="flex-1 overflow-y-auto rounded-lg border border-surface-800  p-5">
		{#if selectedSkill && selectedKey}
			<div class="flex items-center gap-3">
				<img src={getElementIcon(selectedSkill.details.element)} alt="" class="h-8 w-8" />
				<div>
					<h2 class="text-2xl font-bold">{selectedSkill.localized_name}</h2>
					<span class="text-sm" style="color: {elementColor(selectedSkill.details.element)}">{selectedSkill.details.element}</span>
				</div>
			</div>

			<p class="mt-3 text-surface-300">{selectedSkill.description}</p>

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
