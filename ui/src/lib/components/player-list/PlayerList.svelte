<script lang="ts">
	import type { Player, PlayerSummary } from '$types';
	import { Combobox } from '$components/ui';
	import Stopwatch from '$components/ui/stopwatch/Stopwatch.svelte';
	import { getAppState } from '$states';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { goto } from '$app/navigation';

	const appState = getAppState();

	let stopwatchSeconds = $state(0);
	let stopwatchInterval: ReturnType<typeof setInterval> | null = null;

	let {
		selected,
		onselect,
		...additionalProps
	}: {
		selected?: string;
		onselect?: (player: Player) => void;
		[key: string]: any;
	} = $props();

	const selectOptions = $derived.by(() => {
		return Object.entries(appState.playerSummaries as Record<string, PlayerSummary>).map(
			([uid, summary]) => ({
				value: uid,
				label: summary.loaded
					? `🟦 ${summary.nickname || `Player ${uid.slice(0, 8)}`} (${summary.pal_count} pals)`
					: `🟪 ${summary.nickname || `Player ${uid.slice(0, 8)}`} (${summary.pal_count} pals)`
			})
		);
	});

	function handleSelect(playerId: string) {
		if (appState.players[playerId]) {
			onselect?.(appState.players[playerId]);
			appState.selectedPlayer = appState.players[playerId];
			goto('/edit/player');
		} else {
			appState.selectPlayerLazy(playerId);
		}
	}

	$effect(() => {
		const playerCount = Object.keys(appState.playerSummaries).length;

		if (!selected && playerCount === 1) {
			const singlePlayerId = Object.keys(appState.playerSummaries)[0];
			selected = singlePlayerId;
			handleSelect(singlePlayerId);
		}
	});

	$effect(() => {
		if (appState.selectedPlayer && !appState.loadingPlayer) {
			if (selected === appState.selectedPlayer.uid) {
				onselect?.(appState.selectedPlayer);
			}
		}
	});

	$effect(() => {
		if (appState.loadingPlayer) {
			stopwatchSeconds = 0;
			stopwatchInterval = setInterval(() => {
				stopwatchSeconds++;
			}, 1000);
		} else {
			if (stopwatchInterval) {
				clearInterval(stopwatchInterval);
				stopwatchInterval = null;
			}
		}

		return () => {
			if (stopwatchInterval) {
				clearInterval(stopwatchInterval);
				stopwatchInterval = null;
			}
		};
	});
</script>

<div class="w-full" {...additionalProps}>
	{#if appState.loadingPlayer}
		<div class="my-2 flex items-center gap-2 px-3 py-2 text-sm text-gray-400">
			<svg class="h-4 w-4 animate-spin" viewBox="0 0 24 24">
				<circle
					class="opacity-25"
					cx="12"
					cy="12"
					r="10"
					stroke="currentColor"
					stroke-width="4"
					fill="none"
				></circle>
				<path
					class="opacity-75"
					fill="currentColor"
					d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
				></path>
			</svg>
			{m.loading_entity({ entity: c.player })}...
			<Stopwatch bind:seconds={stopwatchSeconds} size="text-sm" />
		</div>
	{:else}
		<Combobox
			value={selected}
			options={selectOptions}
			placeholder={m.select_entity({ entity: c.player })}
			onChange={(value) => handleSelect(value as string)}
			selectClass="w-full"
		/>
	{/if}
</div>
