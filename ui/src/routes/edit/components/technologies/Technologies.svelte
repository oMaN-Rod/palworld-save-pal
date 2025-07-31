<script lang="ts">
	import { getAppState, getModalState } from '$states';
	import { buildingsData, itemsData, technologiesData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { NumberInputModal } from '$components';
	import { Tooltip } from '$components/ui';
	import { Lock, Unlock } from 'lucide-svelte';
	import { EntryState, type Technology } from '$types';
	import { staticIcons } from '$types/icons';

	const appState = getAppState();
	const modal = getModalState();

	const techPointIcon = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/t_icon_tech_point.webp`);
	const ancientTechPointIcon = assetLoader.loadImage(
		`${ASSET_DATA_PATH}/img/t_icon_ancient_tech_point.webp`
	);

	// Order technologies by level cap
	const technologiesOrder = Object.entries(technologiesData.technologies).reduce(
		(acc, [techID, techData]) => {
			const levelCap = techData.details.level_cap;
			if (!acc[levelCap]) {
				acc[levelCap] = {
					regular: [],
					ancient: null
				};
			}

			if (techData.details.is_boss_technology) {
				acc[levelCap].ancient = techID;
			} else {
				acc[levelCap].regular.push(techID);
			}
			return acc;
		},
		{} as Record<number, { regular: string[]; ancient: string | null }>
	);

	function toggleTechnology(techID: string) {
		if (!appState.selectedPlayer) return;
		if (appState.selectedPlayer.technologies.includes(techID)) {
			appState.selectedPlayer.technologies = appState.selectedPlayer.technologies.filter(
				(id) => id !== techID
			);
		} else {
			appState.selectedPlayer.technologies.push(techID);
		}
		appState.selectedPlayer.state = EntryState.MODIFIED;
	}

	async function getImageForTechnology(techID: string): Promise<string | undefined> {
		let techIcon = technologiesData.technologies[techID].details.icon;
		if (!techIcon) {
			return;
		}
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${techIcon}.webp`);
	}

	function resetAll() {
		if (!appState.selectedPlayer) return;
		appState.selectedPlayer.technologies = [];
		appState.selectedPlayer.state = EntryState.MODIFIED;
	}

	function unlockAll() {
		if (!appState.selectedPlayer) return;
		appState.selectedPlayer.technologies = Object.keys(technologiesData.technologies);
		appState.selectedPlayer.state = EntryState.MODIFIED;
	}

	async function handleEditTechPoints(type: 'tech' | 'ancient') {
		if (!appState.selectedPlayer) return;

		const title = type === 'tech' ? 'Technology Points' : 'Ancient Technology Points';
		// @ts-ignore
		const result = await modal.showModal<number>(NumberInputModal, {
			title: title,
			value:
				type === 'tech'
					? appState.selectedPlayer.technology_points
					: appState.selectedPlayer.boss_technology_points,
			min: 0,
			max: 99999
		});
		if (!result) return;

		if (type === 'tech') {
			appState.selectedPlayer.technology_points = result;
		} else {
			appState.selectedPlayer.boss_technology_points = result;
		}
		appState.selectedPlayer.state = EntryState.MODIFIED;
	}
</script>

{#snippet technologyButton(
	techID: string,
	isSelected: any,
	technologyItem: Technology,
	type: 'tech' | 'ancient'
)}
	{@const borderClass = type === 'tech' ? 'border-tech-500' : 'border-ancient-tech-500'}
	{@const backgroundGradient =
		type === 'tech'
			? `bg-linear-to-tl from-tech-500/100 to-tech-500/25`
			: `bg-linear-to-tl from-ancient-tech-500/100 to-ancient-tech-500/25`}
	{@const headerText =
		technologyItem.details.unlock_build_objects.length > 0 ? 'Structures' : 'Items'}
	{@const selectedClass = isSelected
		? ``
		: type === 'tech'
			? 'hover:ring-2 hover:ring-tech-500'
			: 'hover:ring-2 hover:ring-ancient-tech-500'}
	<button
		class="relative h-24 w-24 cursor-pointer border transition-all duration-200 2xl:h-32 2xl:w-32 {borderClass} {selectedClass}"
		onclick={() => toggleTechnology(techID)}
	>
		<Tooltip popupClass="bg-surface-800 p-4">
			<div
				class="mb-2 flex aspect-square items-center justify-center {backgroundGradient} {isSelected
					? ''
					: 'opacity-25'}"
			>
				<div class="absolute top-0 h-auto w-full bg-[#091f35] py-0.5 text-xs">
					{headerText}
				</div>
				{#await getImageForTechnology(techID) then icon}
					<img
						src={icon || staticIcons.unknownIcon}
						alt={technologiesData.technologies[techID].localized_name}
						class="mb-2 h-16 w-16 2xl:h-24 2xl:w-24"
					/>
				{/await}
				<div class="bg-surface-800/50 absolute bottom-0 h-auto w-full py-2 text-xs">
					{technologyItem.localized_name}
				</div>
			</div>
			{#if !isSelected}
				<div class="absolute bottom-6 right-8 2xl:bottom-12 2xl:right-14">
					<h2 class="h2">{technologyItem.details.cost}</h2>
				</div>
			{/if}
			{#snippet popup()}
				<div class="flex min-w-96 max-w-3xl flex-col items-start justify-items-start space-y-2">
					<div class="flex w-full text-start">
						<span class="grow text-xl font-bold">{technologyItem.localized_name}</span>
						<div class="flex items-center">
							<img
								src={type === 'tech' ? techPointIcon : ancientTechPointIcon}
								alt="Tech Point Icon"
								class="mr-2 h-6 w-6"
								loading="lazy"
							/>
							<span>{technologyItem.details.cost}</span>
						</div>
					</div>

					{#if technologyItem.description}
						<div class="text-start">{technologyItem.description}</div>
					{/if}
					{#if technologyItem.details.unlock_build_objects.length > 1}
						<span class="font-bold">Unlocks:</span>
						<ul class="ml-4 list-disc">
							{#each technologyItem.details.unlock_build_objects as buildObject}
								{@const buildingData = buildingsData.getByKey(buildObject)}
								{#if buildingData}
									{@const icon = assetLoader.loadImage(
										`${ASSET_DATA_PATH}/img/${buildingData.icon}.webp`
									)}
									<li class="flex items-center gap-2 text-start">
										<img
											src={icon || staticIcons.unknownIcon}
											alt={buildingData.localized_name}
											class="h-6 w-6"
											loading="lazy"
										/>
										{buildingData.localized_name}
									</li>
								{:else}
									<li>{buildObject}</li>
								{/if}
							{/each}
						</ul>
					{/if}
					{#if technologyItem.details.unlock_item_recipes.length > 1}
						<span class="font-bold">Unlocks:</span>
						<ul class="ml-4 list-disc">
							{#each technologyItem.details.unlock_item_recipes as itemRecipe}
								{@const itemData = itemsData.getByKey(itemRecipe)}
								{#if itemData}
									<li>{itemData.info.localized_name}</li>
								{:else}
									<li>{itemRecipe}</li>
								{/if}
							{/each}
						</ul>
					{/if}
				</div>
			{/snippet}
		</Tooltip>
	</button>
{/snippet}

{#if appState.selectedPlayer}
	<main class=" h-full min-h-screen overflow-y-auto p-8">
		<div class="mx-auto max-w-7xl">
			<div class="mb-8 flex items-center justify-between">
				<div class="flex gap-8">
					<button
						onclick={() => handleEditTechPoints('tech')}
						class="border-surface-400 hover:ring-tech-500 cursor-pointer rounded-lg border hover:ring-2"
					>
						<div class="px-6 py-3">
							<div class="text-surface-400 text-xs">Technology Points</div>
							<div class="text-tech-500 text-2xl font-bold">
								{appState.selectedPlayer.technology_points}
							</div>
						</div>
					</button>
					<button
						onclick={() => handleEditTechPoints('ancient')}
						class="border-surface-400 hover:ring-ancient-tech-500 cursor-pointer rounded-lg border hover:ring-2"
					>
						<div class="px-6 py-3">
							<div class="text-surface-400 text-xs">Ancient Technology Points</div>
							<div class="text-ancient-tech-500 text-2xl font-bold">
								{appState.selectedPlayer.boss_technology_points}
							</div>
						</div>
					</button>
				</div>
				<div class="flex gap-4">
					<button
						onclick={resetAll}
						class="btn preset-filled-primary-500 hover:ring-secondary-500 hover:preset-filled-secondary-500 rounded-lg bg-opacity-20 px-6 py-2 font-medium hover:ring-2"
					>
						<Lock class="inline h-4 w-4" /> Lock All
					</button>
					<button
						onclick={unlockAll}
						class="btn preset-filled-primary-500 hover:ring-secondary-500 hover:preset-filled-secondary-500 rounded-lg bg-opacity-20 px-6 py-2 font-medium hover:ring-2"
					>
						<Unlock class="inline h-4 w-4" /> Unlock All
					</button>
				</div>
			</div>

			{#each Object.entries(technologiesOrder) as [levelCap, levelData]}
				{@const techIDs = levelData.regular}
				{@const emptySlots = 8 - techIDs.length}
				{@const ancientTechID = levelData.ancient}
				<div class="mb-12 grid grid-cols-[auto_1fr] gap-4">
					<div class="mb-4 flex items-center px-10">
						<div
							class="border-surface-500 flex h-12 w-12 items-center justify-center rounded border-2 text-xl font-bold"
						>
							{levelCap}
						</div>
						<div class="ml-4 h-0.5 flex-1"></div>
					</div>

					<div class="flex gap-4">
						<div class="flex gap-4">
							{#each techIDs as techID}
								{@const technologyItem = technologiesData.technologies[techID]}
								{@const isSelected =
									Number(levelCap) === 1
										? true
										: appState.selectedPlayer.technologies.includes(techID)}
								{#if !technologyItem.details.is_boss_technology}
									{@render technologyButton(techID, isSelected, technologyItem, 'tech')}
								{/if}
							{/each}
							{#each Array(emptySlots) as _}
								<div class="w-24 2xl:w-32"></div>
							{/each}
						</div>

						<div class="bg-ancient-tech-500 w-px"></div>

						{#if ancientTechID}
							{@const ancientTechItem = technologiesData.technologies[ancientTechID]}
							{@const isSelected = appState.selectedPlayer.technologies.includes(ancientTechID)}
							{@render technologyButton(ancientTechID, isSelected, ancientTechItem, 'ancient')}
						{:else}
							<div class="w-24 bg-[#220022] 2xl:w-32"></div>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	</main>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">Select a Player to view technologies 🌐</h2>
	</div>
{/if}
