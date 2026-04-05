<script lang="ts">
	import { palsData, elementsData, activeSkillsData } from '$lib/data';
	import WikiSearch from '$components/docs/WikiSearch.svelte';
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

	let search = $state('');
	let selectedKey = $state<string | null>(null);

	const allPals = $derived(
		Object.entries(palsData.pals)
			.filter(([, pal]) => pal.is_pal && !pal.disabled)
			.sort((a, b) => a[1].pal_deck_index - b[1].pal_deck_index)
	);

	const filteredPals = $derived(
		search
			? allPals.filter(
					([key, pal]) =>
						pal.localized_name.toLowerCase().includes(search.toLowerCase()) ||
						key.toLowerCase().includes(search.toLowerCase())
				)
			: allPals
	);

	const selectedPal = $derived(selectedKey ? palsData.pals[selectedKey] : null);

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
</script>

<div class="flex h-full gap-4">
	<div class="flex w-72 shrink-0 flex-col">
		<div class="mb-3 flex items-center justify-between">
			<h1 class="text-lg font-bold">{c.pal} Wiki</h1>
			<span class="text-surface-400 text-xs">{filteredPals.length}</span>
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
									{@const elementIcon = getElementIcon(skillData?.details.element || '')}
									<Tooltip>
										<div
											class="bg-surface-900 flex items-center gap-2 rounded-md px-3 py-1 text-sm"
										>
											<img src={elementIcon} alt="Element icon" class="h-4 w-4 2xl:h-6 2xl:w-6" />
											<div>
												{skillData?.localized_name || skill}
												<span class="text-surface-400">Lv.{level}</span>
											</div>
										</div>
										{#snippet popup()}
											<div class="max-w-xs rounded-md">
												<p class="text-sm">{skillData?.description}</p>
											</div>
										{/snippet}
									</Tooltip>
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
