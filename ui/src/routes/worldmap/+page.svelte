<script lang="ts">
	import { Map } from '$components';
	import { getAppState } from '$states';
	import { Tooltip } from '$components/ui';
	import { Eye, EyeOff } from 'lucide-svelte';
	import { cn } from '$theme';
	import { worldToMap } from '$components/map/utils';

	const appState = getAppState();

	let showOrigin = $state(true);
	let showPlayers = $state(true);

	const players = $derived(Object.values(appState.players || {}));
	const playerCount = $derived(players.length);
</script>

<div class="grid h-full grid-cols-[25%_1fr] gap-2">
	<div class="flex flex-col gap-4 p-4">
		<div class="flex flex-col gap-2">
			<h1 class="text-2xl font-bold">World Map</h1>
			<p class="text-sm text-gray-500">Explore the world of Palworld.</p>
		</div>

		<div class="flex flex-col gap-4">
			<div class="flex flex-col gap-2">
				<h2 class="text-lg font-bold">Map Controls</h2>

				<div class="bg-surface-800 flex items-center justify-between rounded-sm p-2">
					<div class="flex items-center gap-2">
						<button
							class={cn('btn rounded-full p-2', showOrigin ? 'bg-primary-500' : 'bg-surface-700')}
							onclick={() => (showOrigin = !showOrigin)}
						>
							{#if showOrigin}
								<Eye class="h-5 w-5" />
							{:else}
								<EyeOff class="h-5 w-5" />
							{/if}
						</button>
						<span>Origin Marker</span>
					</div>
					<Tooltip label="Toggle visibility of the origin marker (0,0)">
						<div class="text-xs text-gray-400">?</div>
					</Tooltip>
				</div>

				<div class="bg-surface-800 flex items-center justify-between rounded-sm p-2">
					<div class="flex items-center gap-2">
						<button
							class={cn('btn rounded-full p-2', showPlayers ? 'bg-primary-500' : 'bg-surface-700')}
							onclick={() => (showPlayers = !showPlayers)}
						>
							{#if showPlayers}
								<Eye class="h-5 w-5" />
							{:else}
								<EyeOff class="h-5 w-5" />
							{/if}
						</button>
						<span>Players ({playerCount})</span>
					</div>
					<Tooltip label="Toggle visibility of player markers on the map">
						<div class="text-xs text-gray-400">?</div>
					</Tooltip>
				</div>
			</div>

			{#if showPlayers && playerCount > 0}
				<div class="flex flex-col gap-2">
					<h2 class="text-lg font-bold">Players</h2>
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
			{/if}

			<div class="mt-auto flex flex-col gap-2">
				<p class="text-sm text-gray-500">Click on the map to see detailed coordinates.</p>
			</div>
		</div>
	</div>
	<Map {showOrigin} {showPlayers} />
</div>
