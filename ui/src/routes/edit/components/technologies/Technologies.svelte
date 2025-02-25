<script lang="ts">
	import { getAppState } from '$states';
	import { type ItemContainer } from '$types';
	import { technologiesData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';

	const appState = getAppState();

	let unlockedTechnologies: Set<string> = $state(new Set(appState.selectedPlayer?.technologies || []));

	// Order technologies by level cap
	const technologiesOrder = Object.entries(technologiesData.technologies).reduce((acc, [techID, techData]) => {
		const levelCap = techData.details.level_cap;
		if (!acc[levelCap]) {
			acc[levelCap] = [];
		}
		acc[levelCap].push(techID);
		return acc;
	}, {} as Record<number, string[]>);

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
		console.log(techIcon);
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${techIcon}`);
	}
</script>

{#if appState.selectedPlayer}
	<div class="flex h-full flex-col overflow-auto">
		<div class="ml-2 flex">
			<!-- Main content wrapper -->
			<div class="grid w-full grid-cols-1 gap-4 pr-[420px]">
				{#each Object.entries(technologiesOrder) as [levelCap, techIDs]}
					<div class="flex flex-col">
						<h2 class="text-lg font-bold">Level {levelCap}</h2>
						<div class="grid grid-cols-4 gap-4">
							{#each techIDs as techID}
								<button
									type="button"
									class="p-4 border rounded shadow {unlockedTechnologies.has(techID) ? 'bg-green-100' : ''}"
									onclick={() => toggleTechnology(techID)}
								>
								{#await getImageForTechnology(techID) then icon}
								<img src={icon} alt={technologiesData.technologies[techID].localized_name} class="w-full h-auto mb-2" />
							{/await}
									
									<h3 class="font-semibold">{technologiesData.technologies[techID].localized_name}</h3>
									<p>{technologiesData.technologies[techID].description}</p>
								</button>
							{/each}
						</div>
					</div>
				{/each}
			</div>
		</div>
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">Select a Player to view technologies 🌐</h2>
	</div>
{/if}

<style lang="postcss">
	img {
		opacity: 0;
		animation: fadeIn 0.3s ease-in forwards;
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	img:not([src]) {
		animation: fadeOut 0.3s ease-out forwards;
	}

	@keyframes fadeOut {	
		from {
			opacity: 1;
		}
		to {
			opacity: 0;
		}
	}

	.bg-green-100 {
		background-color: #d4f7dc;
	}
</style>
