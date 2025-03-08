<script lang="ts">
	import { Map } from '$components';
	import { getAppState } from '$states';
	import { Checkbox } from '$components/ui';
	import { worldToMap } from '$components/map/utils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';

	const appState = getAppState();

	let showOrigin = $state(false);
	let showPlayers = $state(true);
	let showBases = $state(true);
	let showFastTravel = $state(true);
	let section = $state(['players']);

	const players = $derived(Object.values(appState.players || {}));
	const playerCount = $derived(players.length);
	const guilds = $derived(Object.values(appState.guilds || {}));
	const bases = $derived.by(() => {
		return Object.values(guilds).reduce(
			(acc, guild) => {
				if (guild.bases) {
					Object.values(guild.bases).forEach((base) => {
						acc[base.id] = base;
					});
				}
				return acc;
			},
			{} as Record<string, any>
		);
	});
</script>

<div class="grid h-full grid-cols-[25%_1fr] gap-2">
	<div class="flex flex-col gap-4 p-4">
		<div class="flex flex-col gap-2">
			<h1 class="text-2xl font-bold">World Map</h1>
			<p class="text-sm text-gray-500">Explore the world of Palworld.</p>
		</div>

		<div class="flex flex-col gap-4">
			{#if appState.saveFile}
				<div class="flex flex-col gap-2">
					<h2 class="text-lg font-bold">Map Controls</h2>
					<div class="grid grid-cols-2 gap-2">
						<Checkbox label="Origin" bind:checked={showOrigin} />
						<Checkbox label="Players" bind:checked={showPlayers} />
						<Checkbox label="Bases" bind:checked={showBases} />
						<Checkbox label="Fast Travel" bind:checked={showFastTravel} />
					</div>
				</div>
				<Accordion value={section} onValueChange={(e) => (section = e.value)} collapsible>
					<Accordion.Item value="players" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							<h2 class="text-lg font-bold">Players</h2>
						{/snippet}
						{#snippet panel()}
							{#if showPlayers && playerCount > 0}
								<div class="flex flex-col gap-2">
									<div class="max-h-64 overflow-y-auto">
										{#each players as player}
											{#if player.location}
												{@const mapCoords = worldToMap(player.location.x, player.location.y)}
												<div class="bg-surface-800 mb-2 rounded-sm p-2">
													<div class="font-bold">{player.nickname}</div>
													<div class="text-xs">Level: {player.level} | HP: {player.hp}</div>
													<div class="text-xs text-gray-400">
														Location: {Math.round(mapCoords.x)}, {Math.round(mapCoords.y)}
													</div>
												</div>
											{/if}
										{/each}
									</div>
								</div>
							{:else}
								<p class="text-sm text-gray-500">No players found.</p>
							{/if}
						{/snippet}
					</Accordion.Item>
					<Accordion.Item value="bases" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							<h2 class="text-lg font-bold">Bases</h2>
						{/snippet}
						{#snippet panel()}
							{#if showBases}
								<div class="flex flex-col gap-2">
									<div class="max-h-64 overflow-y-auto">
										{#each Object.values(bases) as base}
											<div class="bg-surface-800 mb-2 rounded-sm p-2">
												<div class="font-bold">{base.id}</div>
												<div class="text-xs text-gray-400">
													Location: {worldToMap(base.location.x, base.location.y).x}, {worldToMap(
														base.location.x,
														base.location.y
													).y}
												</div>
											</div>
										{/each}
									</div>
								</div>
							{:else}
								<p class="text-sm text-gray-500">No bases found.</p>
							{/if}
						{/snippet}
					</Accordion.Item>
				</Accordion>
			{/if}

			<div class="mt-auto flex flex-col gap-2">
				<p class="text-sm text-gray-500">Click on the map to see detailed coordinates.</p>
			</div>
		</div>
	</div>
	<Map {showOrigin} {showPlayers} {showBases} {showFastTravel} />
</div>
