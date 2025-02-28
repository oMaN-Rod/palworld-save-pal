<script lang="ts">
	import type { Player } from '$types';
	import { Combobox } from '$components/ui';
	import { getAppState } from '$states';

	let appState = getAppState();

	let { ...additionalProps } = $props<{
		[key: string]: any;
	}>();

	const selectOptions = $derived(
		Object.entries(appState.players as Record<string, Player>).map(([uid, player]) => ({
			value: uid,
			label: player.nickname || `Player ${uid}`
		}))
	);

	$effect(() => {
		appState.selectedPlayer = appState.players[appState.selectedPlayerUid];
	});
</script>

<div class="w-full" {...additionalProps}>
	<Combobox
		options={selectOptions}
		bind:value={appState.selectedPlayerUid}
		placeholder="Select Player"
	/>
</div>
