<script lang="ts">
	import { getAppState, getSocketState } from '$states';
	import { technologiesData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { MessageType } from '$types';
	
	const appState = getAppState();
	const ws = getSocketState();

	let unlockedTechnologies: Set<string> = $state(new Set<string>());
	let techPoints: number = $state(0);
	let ancientTechPoints: number = $state(0);

	$effect(() => {
		if (appState.selectedPlayer) {
			unlockedTechnologies = new Set(appState.selectedPlayer.technologies);
			techPoints = appState.selectedPlayer.technology_points;
			ancientTechPoints = appState.selectedPlayer.boss_technology_points;
		}
	});

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
		if (unlockedTechnologies.has(techID)) {
			unlockedTechnologies.delete(techID);
		} else {
			unlockedTechnologies.add(techID);
		}
		unlockedTechnologies = new Set(unlockedTechnologies); // Trigger reactivity
	}

	async function getImageForTechnology(techID: string): Promise<string | undefined> {
		let techIcon = technologiesData.technologies[techID].details.icon;
		if (!techIcon) {
			return;
		}
		// If it doesn't end with .png, add it
		if (!techIcon.endsWith('.png')) {
			techIcon += '.png';
		}
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${techIcon}`);
	}

	function resetAll() {
		unlockedTechnologies = new Set();
	}

	function unlockAll() {
		unlockedTechnologies = new Set(Object.keys(technologiesData.technologies));
	}

	async function saveTechnologies() {
		if (!appState.selectedPlayer || !appState.selectedPlayer.technologies) return;

		const message = {
			type: MessageType.SET_TECHNOLOGY_DATA,
			data: {
				playerID: appState.selectedPlayer.uid,
				technologies: Array.from(unlockedTechnologies),
				techPoints: techPoints,
				ancientTechPoints: ancientTechPoints
			}
		};
		const response = await ws.sendAndWait(message);
		if (response.type === 'error') {
			throw new Error(response.data);
		}

		appState.selectedPlayer.technologies = Array.from(unlockedTechnologies);
		appState.selectedPlayer.technology_points = techPoints;
		appState.selectedPlayer.boss_technology_points = ancientTechPoints;
	}

</script>

{#if appState.selectedPlayer}
	<main class="bg-tech-bg h-full min-h-screen overflow-y-auto p-8">
		<div class="mx-auto max-w-7xl">
			<div class="mb-8 flex items-center justify-between">
				<div class="flex gap-8">
					<div class="bg-tech-bg border-tech-border rounded-lg border px-6 py-3">
						<div class="text-xs text-gray-400">Technology Points</div>
						<div class="text-tech-border text-2xl font-bold">{techPoints}</div>
					</div>
					<div class="bg-ancient-bg border-ancient-border rounded-lg border px-6 py-3">
						<div class="text-xs text-gray-400">Ancient Technology Points</div>
						<div class="text-ancient-border text-2xl font-bold">{ancientTechPoints}</div>
					</div>
				</div>
				<div class="flex gap-4">
					<button
						onclick={resetAll}
						class="rounded-lg border border-red-500 bg-red-900 bg-opacity-20 px-6 py-2 font-medium
				   text-red-500 transition-all duration-200 hover:bg-opacity-30"
					>
						Lock All
					</button>
					<button
						onclick={unlockAll}
						class="bg-tech-border text-tech-border border-tech-border rounded-lg border bg-opacity-20 px-6 py-2
				   font-medium transition-all duration-200 hover:bg-opacity-30"
					>
						Unlock All
					</button>
					<button
						onclick={saveTechnologies}
						class="bg-green-500 text-white rounded-lg border border-green-500 px-6 py-2
				   font-medium transition-all duration-200 hover:bg-green-600"
					>
						Save Technologies
					</button>
				</div>
			</div>

			{#each Object.entries(technologiesOrder) as [levelCap, levelData]}
				{@const techIDs = levelData.regular}
				{@const ancientTechID = levelData.ancient}
				<div class="mb-12">
					<div class="mb-4 flex items-center">
						<div
							class="bg-tech-bg border-tech-border text-tech-border flex h-12
						w-12 items-center justify-center rounded border-2 text-xl font-bold"
						>
							{levelCap}
						</div>
						<div class="bg-tech-border ml-4 h-0.5 flex-1"></div>
					</div>

					<div class="flex gap-4">
						<div class="grid flex-1 grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-5">
							{#each techIDs as techID}
								{@const technologyItem = technologiesData.technologies[techID]}
								{@const isSelected = unlockedTechnologies.has(techID)}
								{#if !technologyItem.details.is_boss_technology}
									<button
										class="bg-tech-bg border-tech-border
						 cursor-pointer rounded-lg border p-4 transition-all duration-200
						 {isSelected ? 'border-opacity-100 bg-opacity-20' : 'border-opacity-50 hover:border-opacity-100 opacity-50'}"
										onclick={() => toggleTechnology(techID)}
									>
										<div
											class="relative mb-2 flex aspect-square items-center justify-center rounded-lg {isSelected ? 'bg-[#004444]' : 'bg-[#002222]'} text-4xl"
										>
											{#await getImageForTechnology(techID) then icon}
												<img
													src={icon}
													alt={technologiesData.technologies[techID].localized_name}
													class="mb-2 h-auto w-full"
												/>
											{/await}
											{#if isSelected}
												<div
													class="bg-tech-border absolute right-2 top-2 h-3 w-3 rounded-full"
												></div>
											{/if}
										</div>
										<div class="text-xs text-gray-400">Cost: {technologyItem.details.cost}</div>
										<div class="mt-1 text-sm font-medium text-white">
											{technologyItem.localized_name}
										</div>
									</button>
								{/if}
							{/each}
						</div>

						<div class="bg-ancient-border w-px"></div>

						{#if ancientTechID}
							{@const ancientTechItem = technologiesData.technologies[ancientTechID]}
							{@const isSelected = unlockedTechnologies.has(ancientTechID)}
							<div class="w-64">
								<button
									class="bg-ancient-bg border-ancient-border
					   cursor-pointer rounded-lg border p-4 transition-all duration-200
					   {isSelected ? 'border-opacity-100' : 'border-opacity-50 hover:border-opacity-100 bg-opacity-20 opacity-50'}"
									onclick={() => toggleTechnology(ancientTechID)}
								>
									<div
										class="relative mb-2 flex aspect-square items-center justify-center rounded-lg {isSelected ? 'bg-[#440044]' : 'bg-[#220022]'} text-4xl"
									>
										{#await getImageForTechnology(ancientTechID) then icon}
											<img
												src={icon}
												alt={ancientTechItem.localized_name}
												class="mb-2 h-auto w-full"
											/>
										{/await}
										{#if isSelected}
											<div
												class="bg-ancient-border absolute right-2 top-2 h-3 w-3 rounded-full"
											></div>
										{/if}
									</div>
									<div class="text-xs text-gray-400">{ancientTechItem.details.cost}</div>
									<div class="mt-1 text-sm font-medium text-white">
										{ancientTechItem.localized_name}
									</div>
								</button>
							</div>
						{:else}
							<div class="w-64">
								<div
									class="bg-ancient-bg border-ancient-border
				   cursor-pointer rounded-lg border p-4 transition-all duration-200 opacity-50"
								>
									<div
										class="relative mb-2 flex aspect-square items-center justify-center rounded-lg bg-[#220022] text-4xl"
									></div>
								</div>
							</div>
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
